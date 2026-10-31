"""``scripts/migrate_v1_to_v2.py`` 的回归测试.

运行方式::

    python -m unittest discover -s scripts/tests -v

测试依赖仓库中的 v2 官方数据 ``data/official`` 与
``scripts/v1_official_manifest.json``.
"""

from __future__ import annotations

import contextlib
import io
import json
import sqlite3
import sys
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SCRIPTS_DIR = Path(__file__).resolve().parents[1]
OFFICIAL_DATA = REPO_ROOT / "data" / "official"
MANIFEST_PATH = SCRIPTS_DIR / "v1_official_manifest.json"

sys.path.insert(0, str(SCRIPTS_DIR))
import migrate_v1_to_v2 as migration  # noqa: E402


def tag(namespace: str, value: str) -> dict[str, str]:
    return {"namespace": namespace, "value": value}


def write_json(path: Path, value) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def read_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def run_migration(args: list[str]) -> int:
    with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
        return migration.main(args)


def load_manifest() -> dict:
    return read_json(MANIFEST_PATH)


def manifest_global_id(kind: str, old_id: int, version: str = "main") -> str:
    entry = load_manifest()["versions"][version][kind][str(old_id)]
    if not entry.get("global_id"):
        raise AssertionError(f"manifest 中没有 {kind} {old_id} 的 v2 对应对象")
    return entry["global_id"]


def create_v1_states_db(path: Path, rows: list[tuple[int, int, int, str | None]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(path)
    try:
        conn.execute(
            """
            CREATE TABLE instance_states (
                user_id         INTEGER NOT NULL,
                banner_id       INTEGER NOT NULL,
                total_counter   INTEGER NOT NULL DEFAULT 0,
                logic_state     Text,
                PRIMARY KEY     (user_id, banner_id)
            )
            """
        )
        conn.executemany(
            "INSERT INTO instance_states (user_id, banner_id, total_counter, logic_state) "
            "VALUES (?, ?, ?, ?)",
            rows,
        )
        conn.commit()
    finally:
        conn.close()


def find_local_object(out_data: Path, kind: str, predicate) -> dict | None:
    for path in sorted((out_data / "local" / kind).glob("*.json")):
        obj = read_json(path)
        if predicate(obj):
            return obj
    return None


class MigrationTests(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        self.v1 = self.root / "v1"
        self.out_data = self.root / "out_data"
        self.out_db = self.root / "out_db" / "states.db"

    def tearDown(self) -> None:
        self._tmp.cleanup()

    # -- fixture helpers -------------------------------------------------

    def write_card(self, old_id: int, content: str, tags: list[dict[str, str]]) -> None:
        write_json(
            self.v1 / "cards" / "genshin" / "character" / "5" / f"{old_id}_{content}.json",
            {"id": old_id, "content": content, "tags": tags},
        )

    def write_logic(self, old_id: int, name: str, executor: str) -> None:
        write_json(
            self.v1 / "logics" / f"{old_id}.json",
            {
                "id": old_id,
                "name": name,
                "variant": {"variant": "Hardcoded", "config": {"executor_name": executor}},
            },
        )

    def write_deck(self, old_id: int, name: str, deck: dict) -> None:
        write_json(self.v1 / "decks" / f"{old_id}.json", {"id": old_id, "name": name, **deck})

    def write_banner(self, old_id: int, name: str, banner: dict) -> None:
        write_json(self.v1 / "banners" / f"{old_id}.json", {"id": old_id, "name": name, **banner})

    def official_card(self) -> str:
        return manifest_global_id("cards", 162)

    def official_logic(self) -> str:
        return manifest_global_id("logics", 1)

    # -- tests -----------------------------------------------------------

    def test_v1a_static_deck_and_inline_state(self) -> None:
        """v0.1.0/v0.1.1: 静态卡组列表 + 卡池内联状态."""
        self.write_card(
            162,
            "胡桃",
            [
                tag("game", "genshin"),
                tag("rarity", "5"),
                tag("type", "character"),
                tag("profession", "长柄武器"),
                tag("attribute", "火"),
            ],
        )
        self.write_card(
            9001,
            "测试角色",
            [
                tag("game", "genshin"),
                tag("rarity", "5"),
                tag("type", "character"),
                tag("profession", "单手剑"),
                tag("attribute", "冰"),
            ],
        )
        self.write_logic(1, "原神角色 UP 池逻辑", "genshin_character_up")
        self.write_deck(
            1,
            "我的静态卡组",
            {
                "members": [162, 9001],
                "event_groups": {"up": [9001]},
                "tags": [tag("game", "原神"), tag("event", "UP")],
            },
        )
        self.write_banner(
            1,
            "我的静态卡池",
            {
                "tags": [tag("game", "原神"), tag("event", "UP")],
                "deck_id": 1,
                "logic_instance": {
                    "logic_id": 1,
                    "state": {
                        "total_counter": 42,
                        "counter_5": 10,
                        "counter_4": 2,
                        "is_pity_5": True,
                        "is_pity_4": False,
                        "next_char_4": False,
                    },
                },
            },
        )

        code = run_migration(
            [
                "--data-dir", str(self.v1),
                "--official-data", str(OFFICIAL_DATA),
                "--out-data", str(self.out_data),
            ]
        )
        self.assertEqual(code, 0)

        default_db = self.root / "db_v2" / "states.db"
        self.assertTrue(default_db.is_file())

        report = read_json(self.out_data / "migration-report.json")
        self.assertEqual(report["warnings"], [])
        self.assertEqual(report["stats"]["kinds"]["cards"]["official"], 1)
        self.assertEqual(report["stats"]["kinds"]["cards"]["local"], 1)
        self.assertEqual(report["stats"]["kinds"]["decks"]["local"], 1)
        self.assertEqual(report["stats"]["kinds"]["banners"]["local"], 1)
        self.assertEqual(report["stats"]["kinds"]["logics"]["official"], 1)
        self.assertEqual(report["stats"]["states_migrated"], 1)

        self.assertEqual(read_json(self.out_data / "version.json")["format_version"], 2)
        self.assertTrue((self.out_data / "official" / "cards").is_dir())

        custom_card = find_local_object(
            self.out_data, "cards", lambda obj: obj["content"].get("zh-CN") == "测试角色"
        )
        self.assertIsNotNone(custom_card)
        custom_tags = {(item["namespace"], item["value"]) for item in custom_card["tags"]}
        self.assertIn(("profession", "sword"), custom_tags)
        self.assertIn(("attribute", "cryo"), custom_tags)
        self.assertNotIn("forked_from", custom_card)

        deck = find_local_object(self.out_data, "decks", lambda obj: obj["name"].get("zh-CN") == "我的静态卡组")
        self.assertIsNotNone(deck)
        include_condition = deck["members"]["conditions"][0]
        self.assertEqual(include_condition["type"], "include_ids")
        self.assertEqual(
            set(include_condition["ids"]),
            {self.official_card(), custom_card["global_id"]},
        )
        up_condition = deck["event_groups"]["up"]["conditions"][0]
        self.assertEqual(set(up_condition["ids"]), {custom_card["global_id"]})
        deck_tags = {(item["namespace"], item["value"]) for item in deck["tags"]}
        self.assertIn(("game", "genshin"), deck_tags)
        self.assertIn(("event", "up"), deck_tags)

        banner = find_local_object(
            self.out_data, "banners", lambda obj: obj["name"].get("zh-CN") == "我的静态卡池"
        )
        self.assertIsNotNone(banner)
        self.assertEqual(banner["deck_id"], deck["global_id"])
        self.assertEqual(banner["logic_id"], self.official_logic())
        self.assertEqual(banner["state_scope"], "banner")

        conn = sqlite3.connect(default_db)
        try:
            rows = conn.execute(
                "SELECT scope_key, user_id, total_counter, logic_state FROM instance_states"
            ).fetchall()
        finally:
            conn.close()
        self.assertEqual(len(rows), 1)
        scope_key, user_id, total_counter, logic_state = rows[0]
        self.assertEqual(scope_key, f"banner:{banner['global_id']}")
        self.assertEqual(user_id, 0)
        self.assertEqual(total_counter, 42)
        state = json.loads(logic_state)
        self.assertEqual(state["counter_5"], 10)
        self.assertNotIn("total_counter", state)

    def test_v1b_user_objects_forks_and_references(self) -> None:
        """v0.1.2/main: 用户对象、官方卡片派生、引用重写、数据库状态."""
        official_tags = [
            tag("game", "genshin"),
            tag("rarity", "5"),
            tag("type", "character"),
            tag("profession", "长柄武器"),
            tag("attribute", "火"),
        ]
        self.write_card(162, "胡桃", official_tags)
        self.write_card(
            9001,
            "测试角色",
            [
                tag("game", "genshin"),
                tag("rarity", "5"),
                tag("type", "character"),
                tag("profession", "单手剑"),
                tag("attribute", "冰"),
            ],
        )
        self.write_card(9002, "胡桃", official_tags + [tag("custom", "modified")])
        self.write_logic(1, "原神角色 UP 池逻辑", "genshin_character_up")
        self.write_deck(
            9001,
            "我的规则卡组",
            {
                "members": {
                    "conditions": [
                        {
                            "type": "include_ids",
                            "ids": [162, 9001, 9002],
                        }
                    ]
                },
                "event_groups": {
                    "up": {
                        "conditions": [
                            {"type": "include_ids", "ids": [9001, 9002]},
                        ]
                    }
                },
                "tags": [tag("game", "原神")],
            },
        )
        self.write_banner(
            9001,
            "我的规则卡池",
            {
                "tags": [tag("game", "原神"), tag("event", "UP")],
                "deck_id": 9001,
                "logic_id": 1,
            },
        )
        create_v1_states_db(
            self.root / "v1" / "db" / "states.db",
            [(0, 9001, 42, json.dumps({"counter_5": 30, "is_pity_5": True}))],
        )

        code = run_migration(
            [
                "--data-dir", str(self.v1),
                "--db", str(self.v1 / "db" / "states.db"),
                "--official-data", str(OFFICIAL_DATA),
                "--out-data", str(self.out_data),
                "--out-db", str(self.out_db),
            ]
        )
        self.assertEqual(code, 0)

        report = read_json(self.out_data / "migration-report.json")
        self.assertEqual(report["warnings"], [])
        self.assertEqual(report["stats"]["kinds"]["cards"]["official"], 1)
        self.assertEqual(report["stats"]["kinds"]["cards"]["fork"], 1)
        self.assertEqual(report["stats"]["kinds"]["cards"]["local"], 1)
        self.assertEqual(report["stats"]["states_migrated"], 1)

        custom_card = find_local_object(
            self.out_data, "cards", lambda obj: obj["content"].get("zh-CN") == "测试角色"
        )
        self.assertIsNotNone(custom_card)
        self.assertNotIn("forked_from", custom_card)

        fork = find_local_object(
            self.out_data,
            "cards",
            lambda obj: obj.get("forked_from") == self.official_card(),
        )
        self.assertIsNotNone(fork)
        self.assertEqual(fork["origin"], "local")
        fork_tags = {(item["namespace"], item["value"]) for item in fork["tags"]}
        self.assertIn(("custom", "modified"), fork_tags)

        deck = find_local_object(
            self.out_data, "decks", lambda obj: obj["name"].get("zh-CN") == "我的规则卡组"
        )
        self.assertIsNotNone(deck)
        include_ids = set(deck["members"]["conditions"][0]["ids"])
        self.assertEqual(
            include_ids,
            {self.official_card(), custom_card["global_id"], fork["global_id"]},
        )
        up_ids = set(deck["event_groups"]["up"]["conditions"][0]["ids"])
        self.assertEqual(up_ids, {custom_card["global_id"], fork["global_id"]})

        banner = find_local_object(
            self.out_data, "banners", lambda obj: obj["name"].get("zh-CN") == "我的规则卡池"
        )
        self.assertIsNotNone(banner)
        self.assertEqual(banner["deck_id"], deck["global_id"])
        self.assertEqual(banner["logic_id"], self.official_logic())

        conn = sqlite3.connect(self.out_db)
        try:
            row = conn.execute(
                "SELECT scope_key, total_counter, logic_state FROM instance_states"
            ).fetchone()
        finally:
            conn.close()
        self.assertIsNotNone(row)
        self.assertEqual(row[0], f"banner:{banner['global_id']}")
        self.assertEqual(row[1], 42)
        self.assertEqual(json.loads(row[2])["counter_5"], 30)

    def test_dry_run_writes_nothing(self) -> None:
        self.write_card(9001, "测试角色", [tag("game", "genshin")])

        code = run_migration(
            [
                "--data-dir", str(self.v1),
                "--official-data", str(OFFICIAL_DATA),
                "--out-data", str(self.out_data),
                "--out-db", str(self.out_db),
                "--dry-run",
            ]
        )
        self.assertEqual(code, 0)
        self.assertFalse(self.out_data.exists())
        self.assertFalse(self.out_db.exists())

    def test_existing_output_requires_force(self) -> None:
        self.write_card(9001, "测试角色", [tag("game", "genshin")])
        self.out_data.mkdir(parents=True)
        keep = self.out_data / "keep.txt"
        keep.write_text("keep", encoding="utf-8")

        code = run_migration(
            [
                "--data-dir", str(self.v1),
                "--official-data", str(OFFICIAL_DATA),
                "--out-data", str(self.out_data),
                "--out-db", str(self.out_db),
            ]
        )
        self.assertEqual(code, 1)
        self.assertTrue(keep.exists())

    def test_dry_run_force_does_not_delete_existing_output(self) -> None:
        self.write_card(9001, "测试角色", [tag("game", "genshin")])
        self.out_data.mkdir(parents=True)
        keep = self.out_data / "keep.txt"
        keep.write_text("keep", encoding="utf-8")

        code = run_migration(
            [
                "--data-dir", str(self.v1),
                "--official-data", str(OFFICIAL_DATA),
                "--out-data", str(self.out_data),
                "--out-db", str(self.out_db),
                "--dry-run",
                "--force",
            ]
        )
        self.assertEqual(code, 0)
        self.assertTrue(keep.exists())
        self.assertFalse(self.out_db.exists())

    def test_v1_data_as_official_reference_is_rejected(self) -> None:
        self.write_card(9001, "测试角色", [tag("game", "genshin")])

        code = run_migration(
            [
                "--data-dir", str(self.v1),
                "--official-data", str(self.v1),
                "--out-data", str(self.out_data),
                "--out-db", str(self.out_db),
            ]
        )
        self.assertEqual(code, 1)
        self.assertFalse(self.out_data.exists())


if __name__ == "__main__":
    unittest.main()
