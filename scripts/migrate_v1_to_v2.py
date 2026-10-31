#!/usr/bin/env python3
"""Wishes v1 -> v2 数据迁移脚本.

将 v1 数据目录与 SQLite 状态数据库迁移为 v2 布局:

- v1 对象使用自增整数 Id 与中文标签值;
- v2 对象使用 Uuid v7 GlobalId、多语言文本、``official`` / ``local`` 来源;
- v2 的卡池状态以 ``banner:<uuid>`` 作为 ``scope_key`` 存储.

脚本不会修改输入目录. 它会在输出目录中生成一份完整的 v2 数据目录
(包含 v2 的 ``official/`` 副本、迁移后的 ``local/`` 和 ``version.json``),
以及一份新的 v2 ``states.db``.

用法示例::

    python scripts/migrate_v1_to_v2.py \\
        --data-dir data \\
        --db db/states.db \\
        --official-data data/official \\
        --out-data data_v2 \\
        --out-db db_v2/states.db

若脚本位于 v2 仓库内, ``--official-data`` 默认指向仓库根目录的
``data/official``; 若从别处运行, 请显式传入 v2 应用的数据目录或
``official`` 目录.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import secrets
import shutil
import sqlite3
import sys
import time
import uuid
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Iterable

DATA_FORMAT_VERSION = 2
DEFAULT_LOCALE = "zh-CN"
OBJECT_KINDS = ("cards", "decks", "logics", "banners")
TAG_CONDITION_TYPES = frozenset({"tag_all", "tag_any", "filter_tag_all", "filter_tag_any"})
ID_CONDITION_TYPES = frozenset({"include_ids", "exclude_ids"})

# 已知的 v1 标签值 -> v2 标签值.
#
# 运行时还会从 v2 官方数据与 v1 默认数据的对应关系中自动推导映射,
# 因此这张表主要用于覆盖官方默认数据中未出现、但用户可能使用过的值.
# 未在映射表中的标签会原样保留, 并记录一条警告.
DEFAULT_TAG_MAP: dict[tuple[str, str], tuple[str, str]] = {
    # game
    ("game", "原神"): ("game", "genshin"),
    ("game", "崩铁"): ("game", "starrail"),
    ("game", "崩坏星穹铁道"): ("game", "starrail"),
    ("game", "崩坏:星穹铁道"): ("game", "starrail"),
    ("game", "星穹铁道"): ("game", "starrail"),
    # event
    ("event", "UP"): ("event", "up"),
    ("event", "up"): ("event", "up"),
    ("event", "standard"): ("event", "standard"),
    ("event", "FES"): ("event", "fes"),
    ("event", "fes"): ("event", "fes"),
    ("event", "appoint"): ("event", "appoint"),
    # profession (原神)
    ("profession", "单手剑"): ("profession", "sword"),
    ("profession", "双手剑"): ("profession", "claymore"),
    ("profession", "长柄武器"): ("profession", "polearm"),
    ("profession", "弓"): ("profession", "bow"),
    ("profession", "法器"): ("profession", "catalyst"),
    # attribute (原神元素 / 星铁属性)
    ("attribute", "火"): ("attribute", "pyro"),
    ("attribute", "水"): ("attribute", "hydro"),
    ("attribute", "风"): ("attribute", "anemo"),
    ("attribute", "雷"): ("attribute", "electro"),
    ("attribute", "草"): ("attribute", "dendro"),
    ("attribute", "冰"): ("attribute", "cryo"),
    ("attribute", "岩"): ("attribute", "geo"),
    ("attribute", "物理"): ("attribute", "physical"),
    ("attribute", "虚数"): ("attribute", "imaginary"),
    ("attribute", "量子"): ("attribute", "quantum"),
    # fate (星铁命途)
    ("fate", "存护"): ("fate", "preservation"),
    ("fate", "同谐"): ("fate", "harmony"),
    ("fate", "巡猎"): ("fate", "the_hunt"),
    ("fate", "智识"): ("fate", "erudition"),
    ("fate", "欢愉"): ("fate", "elation"),
    ("fate", "毁灭"): ("fate", "destruction"),
    ("fate", "虚无"): ("fate", "nihility"),
    ("fate", "记忆"): ("fate", "remembrance"),
    ("fate", "丰饶"): ("fate", "abundance"),
}

# 早期 v1 数据可能使用 ``path`` 表示星铁命途, 统一映射到 v2 的 ``fate``.
_PATH_VALUES = {
    "存护": "preservation",
    "同谐": "harmony",
    "巡猎": "the_hunt",
    "智识": "erudition",
    "欢愉": "elation",
    "毁灭": "destruction",
    "虚无": "nihility",
    "记忆": "remembrance",
    "丰饶": "abundance",
}
DEFAULT_TAG_MAP.update({("path", k): ("fate", v) for k, v in _PATH_VALUES.items()})


class MigrationFatal(Exception):
    """无法继续迁移的错误."""


@dataclass
class KindStats:
    read: int = 0
    official: int = 0
    fork: int = 0
    local: int = 0
    skipped: int = 0

    def as_dict(self) -> dict[str, int]:
        return {
            "read": self.read,
            "official": self.official,
            "fork": self.fork,
            "local": self.local,
            "skipped": self.skipped,
        }


@dataclass
class MigrationStats:
    kinds: dict[str, KindStats] = field(
        default_factory=lambda: {kind: KindStats() for kind in OBJECT_KINDS}
    )
    states_migrated: int = 0
    states_skipped: int = 0


@dataclass
class MigrationReport:
    data_dir: str
    db_path: str | None
    official_data: str
    out_data: str
    out_db: str | None
    dry_run: bool
    tag_map: dict[str, str] = field(default_factory=dict)
    objects: dict[str, list[dict[str, Any]]] = field(
        default_factory=lambda: {kind: [] for kind in OBJECT_KINDS}
    )
    stats: MigrationStats = field(default_factory=MigrationStats)
    warnings: list[str] = field(default_factory=list)

    def as_dict(self) -> dict[str, Any]:
        return {
            "data_format": DATA_FORMAT_VERSION,
            "data_dir": self.data_dir,
            "db_path": self.db_path,
            "official_data": self.official_data,
            "out_data": self.out_data,
            "out_db": self.out_db,
            "dry_run": self.dry_run,
            "tag_map": self.tag_map,
            "objects": self.objects,
            "stats": {
                "kinds": {kind: stats.as_dict() for kind, stats in self.stats.kinds.items()},
                "states_migrated": self.stats.states_migrated,
                "states_skipped": self.stats.states_skipped,
            },
            "warnings": self.warnings,
        }


# ---------------------------------------------------------------------------
# 基础工具
# ---------------------------------------------------------------------------


def read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except OSError as exc:
        raise MigrationFatal(f"无法读取 {path}: {exc}") from exc
    except json.JSONDecodeError as exc:
        raise MigrationFatal(f"{path} 不是合法 JSON: {exc}") from exc


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps(value, ensure_ascii=False, indent=2) + "\n"
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(text, encoding="utf-8")
    tmp.replace(path)


def load_objects(directory: Path) -> list[tuple[Path, dict[str, Any]]]:
    """递归加载目录中的全部 JSON 对象."""
    if not directory.is_dir():
        return []
    result: list[tuple[Path, dict[str, Any]]] = []
    for path in sorted(directory.rglob("*.json")):
        value = read_json(path)
        if not isinstance(value, dict):
            raise MigrationFatal(f"{path} 的顶层不是 JSON 对象")
        result.append((path, value))
    return result


def zh_text(value: Any) -> str | None:
    """从 v1 字符串或 v2 LocalizedString 中取出中文文本."""
    if isinstance(value, str):
        return value or None
    if isinstance(value, dict):
        if value.get(DEFAULT_LOCALE):
            return str(value[DEFAULT_LOCALE])
        for key in ("en", "ja"):
            if value.get(key):
                return str(value[key])
        for text in value.values():
            if text:
                return str(text)
    return None


def to_localized(value: Any) -> dict[str, str]:
    """把 v1 文本转换为 v2 的多语言映射."""
    if isinstance(value, str):
        return {DEFAULT_LOCALE: value} if value else {}
    if isinstance(value, dict):
        return {
            str(locale): str(text)
            for locale, text in value.items()
            if isinstance(text, str) and text
        }
    return {}


def tag_pair(tag: dict[str, Any]) -> tuple[str, str]:
    return (str(tag.get("namespace", "")), str(tag.get("value", "")))


def tag_signature(tags: Iterable[dict[str, Any]]) -> tuple[tuple[str, str], ...]:
    return tuple(sorted({tag_pair(tag) for tag in tags}))


def tags_to_json(pairs: Iterable[tuple[str, str]]) -> list[dict[str, str]]:
    return [{"namespace": ns, "value": value} for ns, value in sorted(pairs)]


def game_value(tags: Iterable[dict[str, Any]]) -> str | None:
    for tag in tags:
        if tag.get("namespace") == "game":
            return str(tag.get("value"))
    return None


def uuid7() -> str:
    """生成 Uuid v7 (时间有序) 字符串."""
    ms = int(time.time() * 1000) & ((1 << 48) - 1)
    rand_a = secrets.randbits(12)
    rand_b = secrets.randbits(62)

    raw = bytearray(16)
    raw[0:6] = ms.to_bytes(6, "big")
    raw[6] = 0x70 | ((rand_a >> 8) & 0x0F)
    raw[7] = rand_a & 0xFF
    raw[8] = 0x80 | ((rand_b >> 56) & 0x3F)
    raw[9:16] = (rand_b & ((1 << 56) - 1)).to_bytes(7, "big")
    return str(uuid.UUID(bytes=bytes(raw)))


def canonical_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


# ---------------------------------------------------------------------------
# 标签与条件转换
# ---------------------------------------------------------------------------


def load_tag_map_override(path: Path, warnings: list[str]) -> dict[tuple[str, str], tuple[str, str]]:
    """读取 ``--tag-map`` 提供的额外映射."""
    value = read_json(path)
    if not isinstance(value, dict):
        raise MigrationFatal(f"{path} 必须是形如 {{\"game:原神\": \"game:genshin\"}} 的 JSON 对象")
    result: dict[tuple[str, str], tuple[str, str]] = {}
    for source, target in value.items():
        source_ns, sep, source_value = str(source).partition(":")
        if not sep:
            raise MigrationFatal(f"{path} 中的键 {source!r} 缺少命名空间分隔符 ':'")
        if isinstance(target, str):
            target_ns, sep, target_value = target.partition(":")
            if not sep:
                raise MigrationFatal(f"{path} 中的值 {target!r} 缺少命名空间分隔符 ':'")
        elif isinstance(target, (list, tuple)) and len(target) == 2:
            target_ns, target_value = str(target[0]), str(target[1])
        else:
            raise MigrationFatal(f"{path} 中 {source!r} 的目标值必须是 'ns:value' 或 [ns, value]")
        result[(source_ns, source_value)] = (target_ns, target_value)
    return result


def add_tag_mapping(
    mapping: dict[tuple[str, str], tuple[str, str]],
    source: tuple[str, str],
    target: tuple[str, str],
    warnings: list[str],
    where: str,
) -> None:
    # 内置映射优先: v0.1.0 / v0.1.1 的默认数据存在历史错误
    # (例如流萤的属性写成了"风", v2 已修正为 physical),
    # 不能让旧默认数据覆盖已知映射.
    if source in DEFAULT_TAG_MAP:
        return

    previous = mapping.get(source)
    if previous is None:
        mapping[source] = target
    elif previous != target:
        warnings.append(
            f"标签映射冲突 ({where}): {source[0]}:{source[1]} 已有映射 "
            f"{previous[0]}:{previous[1]}, 忽略 {target[0]}:{target[1]}"
        )


def derive_tag_map(
    v1_cards: list[tuple[Path, dict[str, Any]]],
    v1_decks: list[tuple[Path, dict[str, Any]]],
    v1_banners: list[tuple[Path, dict[str, Any]]],
    v1_logics: list[tuple[Path, dict[str, Any]]],
    v2_cards: list[tuple[Path, dict[str, Any]]],
    v2_decks: list[tuple[Path, dict[str, Any]]],
    v2_banners: list[tuple[Path, dict[str, Any]]],
    v2_logics: list[tuple[Path, dict[str, Any]]],
    warnings: list[str],
) -> dict[tuple[str, str], tuple[str, str]]:
    """从 v1/v2 官方默认数据的对应关系中推导标签映射."""
    mapping = dict(DEFAULT_TAG_MAP)

    # 卡片: 内容 + game 唯一匹配, 再按同名命名空间推导标签值.
    v2_card_index: dict[tuple[str | None, str | None], list[dict[str, Any]]] = defaultdict(list)
    for _, card in v2_cards:
        v2_card_index[(zh_text(card.get("content")), game_value(card.get("tags", [])))].append(card)

    for _, card in v1_cards:
        key = (zh_text(card.get("content")), game_value(card.get("tags", [])))
        candidates = v2_card_index.get(key, [])
        if len(candidates) != 1:
            continue
        official = candidates[0]
        for tag in card.get("tags", []):
            namespace = tag.get("namespace")
            same_namespace = [t for t in official.get("tags", []) if t.get("namespace") == namespace]
            if len(same_namespace) == 1:
                add_tag_mapping(
                    mapping,
                    tag_pair(tag),
                    tag_pair(same_namespace[0]),
                    warnings,
                    f"Card {card.get('id')}",
                )

    # 卡组 / 卡池 / 逻辑: 按名称唯一匹配, 推导其自身标签 (game / event 等).
    for label, v1_objects, v2_objects in (
        ("Deck", v1_decks, v2_decks),
        ("Banner", v1_banners, v2_banners),
        ("Logic", v1_logics, v2_logics),
    ):
        index: dict[str | None, list[dict[str, Any]]] = defaultdict(list)
        for _, obj in v2_objects:
            index[zh_text(obj.get("name"))].append(obj)

        for _, obj in v1_objects:
            candidates = index.get(zh_text(obj.get("name")), [])
            if len(candidates) != 1:
                continue
            official = candidates[0]
            for tag in obj.get("tags", []):
                namespace = tag.get("namespace")
                same_namespace = [t for t in official.get("tags", []) if t.get("namespace") == namespace]
                if len(same_namespace) == 1:
                    add_tag_mapping(
                        mapping,
                        tag_pair(tag),
                        tag_pair(same_namespace[0]),
                        warnings,
                        f"{label} {obj.get('id')}",
                    )

    return mapping


def map_tag_value(
    tag: dict[str, Any],
    tag_map: dict[tuple[str, str], tuple[str, str]],
    warnings: list[str],
    where: str,
) -> tuple[str, str]:
    source = tag_pair(tag)
    target = tag_map.get(source)
    if target is None:
        target = source
        if any(ord(ch) > 127 for ch in source[1]):
            warnings.append(f"{where}: 标签 {source[0]}:{source[1]} 没有映射, 保留原值")
    return target


def map_tags(
    tags: Iterable[dict[str, Any]],
    tag_map: dict[tuple[str, str], tuple[str, str]],
    warnings: list[str],
    where: str,
) -> list[dict[str, str]]:
    mapped: dict[tuple[str, str], None] = {}
    for tag in tags:
        mapped[map_tag_value(tag, tag_map, warnings, where)] = None
    return tags_to_json(mapped.keys())


def map_event_key(
    key: Any,
    tag_map: dict[tuple[str, str], tuple[str, str]],
    warnings: list[str],
    where: str,
) -> str:
    raw = str(key)
    target = tag_map.get(("event", raw))
    if target is not None and target[0] == "event":
        return target[1]
    if raw in {"standard", "up", "fes", "appoint"}:
        return raw
    lowered = raw.lower()
    if lowered in {"standard", "up", "fes", "appoint"}:
        return lowered
    warnings.append(f"{where}: 未知活动标签键 {raw!r}, 保留原值")
    return raw


def convert_condition(
    condition: dict[str, Any],
    card_map: dict[int, str],
    tag_map: dict[tuple[str, str], tuple[str, str]],
    warnings: list[str],
    where: str,
) -> dict[str, Any]:
    kind = str(condition.get("type", ""))
    if kind in ID_CONDITION_TYPES:
        ids: set[str] = set()
        for raw_id in condition.get("ids", []) or []:
            try:
                old_id = int(raw_id)
            except (TypeError, ValueError):
                warnings.append(f"{where}: 非整数卡片 Id {raw_id!r}, 已丢弃")
                continue
            new_id = card_map.get(old_id)
            if new_id:
                ids.add(new_id)
            else:
                warnings.append(f"{where}: 引用的卡片 Id {old_id} 不存在, 已丢弃")
        return {"type": kind, "ids": sorted(ids)}

    if kind in TAG_CONDITION_TYPES:
        return {"type": kind, "tags": map_tags(condition.get("tags", []), tag_map, warnings, where)}

    if kind == "all":
        return {"type": "all"}

    if kind in {"include_groups", "exclude_groups"}:
        warnings.append(f"{where}: v2 暂不支持条件 {kind}, 原样保留")
        return dict(condition)

    warnings.append(f"{where}: 未知条件类型 {kind!r}, 原样保留")
    return dict(condition)


def convert_conditions(
    conditions: Iterable[dict[str, Any]],
    card_map: dict[int, str],
    tag_map: dict[tuple[str, str], tuple[str, str]],
    warnings: list[str],
    where: str,
) -> list[dict[str, Any]]:
    return [
        convert_condition(condition, card_map, tag_map, warnings, f"{where}[{index}]")
        for index, condition in enumerate(conditions)
    ]


# ---------------------------------------------------------------------------
# 签名 (用于判断 v1 对象是否与 v2 官方对象完全一致)
# ---------------------------------------------------------------------------


def condition_signature(condition: dict[str, Any]) -> tuple[Any, ...]:
    kind = str(condition.get("type", ""))
    if kind in ID_CONDITION_TYPES:
        ids = tuple(sorted(str(item) for item in (condition.get("ids", []) or [])))
        return (kind, ids)
    if kind in TAG_CONDITION_TYPES:
        return (kind, tag_signature(condition.get("tags", [])))
    return (kind, canonical_json(condition))


def deck_signature(deck: dict[str, Any]) -> tuple[Any, ...]:
    members = deck.get("members", {}) or {}
    event_groups = deck.get("event_groups", {}) or {}
    return (
        zh_text(deck.get("name")),
        tag_signature(deck.get("tags", [])),
        tuple(condition_signature(item) for item in members.get("conditions", []) or []),
        tuple(
            sorted(
                (str(key), tuple(condition_signature(item) for item in (group or {}).get("conditions", []) or []))
                for key, group in event_groups.items()
            )
        ),
    )


def banner_signature(banner: dict[str, Any]) -> tuple[Any, ...]:
    return (
        zh_text(banner.get("name")),
        tag_signature(banner.get("tags", [])),
        str(banner.get("deck_id")),
        str(banner.get("logic_id")),
        str(banner.get("state_scope", "banner")),
    )


def logic_signature(logic: dict[str, Any]) -> tuple[Any, ...]:
    return (
        zh_text(logic.get("name")),
        canonical_json(logic.get("variant")),
    )


def v1_members_signature(members: Any) -> tuple[Any, ...]:
    """v1 早期 (v0.1.0/v0.1.1) 的 members 是静态 Id 列表, 后期是 conditions."""
    if isinstance(members, list):
        return ("static_ids", tuple(sorted(str(item) for item in members)))
    if isinstance(members, dict):
        return (
            "conditions",
            tuple(condition_signature(item) for item in (members.get("conditions") or [])),
        )
    return ("empty",)


def v1_event_group_signature(group: Any) -> tuple[Any, ...]:
    if isinstance(group, list):
        return ("static_ids", tuple(sorted(str(item) for item in group)))
    if isinstance(group, dict):
        return (
            "conditions",
            tuple(condition_signature(item) for item in (group.get("conditions") or [])),
        )
    return ("empty",)


def v1_banner_logic_id(obj: dict[str, Any]) -> int:
    """v1 早期卡池把 logic_id 放在 logic_instance 中."""
    if obj.get("logic_id") is not None:
        return int(obj["logic_id"])
    instance = obj.get("logic_instance") or {}
    return int(instance.get("logic_id"))


def v1_signature(kind: str, obj: dict[str, Any]) -> tuple[Any, ...]:
    """v1 官方默认对象的稳定签名 (包含旧 Id, 不含任何映射)."""
    old_id = int(obj["id"])
    if kind == "cards":
        return ("card", old_id, zh_text(obj.get("content")), tag_signature(obj.get("tags", [])))
    if kind == "decks":
        event_groups = obj.get("event_groups", {}) or {}
        return (
            "deck",
            old_id,
            zh_text(obj.get("name")),
            tag_signature(obj.get("tags", [])),
            v1_members_signature(obj.get("members")),
            tuple(
                sorted(
                    (str(key), v1_event_group_signature(group))
                    for key, group in event_groups.items()
                )
            ),
        )
    if kind == "banners":
        return (
            "banner",
            old_id,
            zh_text(obj.get("name")),
            tag_signature(obj.get("tags", [])),
            int(obj["deck_id"]),
            v1_banner_logic_id(obj),
        )
    if kind == "logics":
        return ("logic", old_id, zh_text(obj.get("name")), canonical_json(obj.get("variant")))
    raise MigrationFatal(f"未知对象类型: {kind}")


def v1_signature_hash(kind: str, obj: dict[str, Any]) -> str:
    return hashlib.sha256(canonical_json(v1_signature(kind, obj)).encode("utf-8")).hexdigest()


def load_v1_manifest(
    path: Path,
    warnings: list[str],
    enabled: bool,
) -> dict[str, dict[str, list[dict[str, Any]]]]:
    """加载 v1 各版本官方默认对象清单.

    清单结构::

        {
          "format": 1,
          "versions": {
            "v0.1.2": {
              "cards": {"162": {"hash": "...", "global_id": "..."}},
              ...
            }
          }
        }
    """
    result: dict[str, dict[str, list[dict[str, Any]]]] = defaultdict(lambda: defaultdict(list))
    if not enabled:
        return result
    if not path.is_file():
        warnings.append(
            f"未找到 v1 官方清单 {path}, 将采用保守策略: "
            "与 v2 官方数据不同的对象都会保留为本地派生"
        )
        return result

    data = read_json(path)
    if not isinstance(data, dict) or data.get("format") != 1 or not isinstance(data.get("versions"), dict):
        raise MigrationFatal(f"{path} 不是受支持的 v1 官方清单 (format 应为 1)")

    for version, kinds in data["versions"].items():
        if not isinstance(kinds, dict):
            continue
        for kind, objects in kinds.items():
            if kind not in OBJECT_KINDS or not isinstance(objects, dict):
                continue
            for old_id, entry in objects.items():
                if isinstance(entry, str):
                    entry = {"hash": entry, "global_id": None}
                if not isinstance(entry, dict) or not isinstance(entry.get("hash"), str):
                    raise MigrationFatal(f"{path}: {version}/{kind}/{old_id} 的清单条目无效")
                result[kind][str(old_id)].append(
                    {
                        "hash": entry["hash"],
                        "global_id": entry.get("global_id"),
                        "version": version,
                    }
                )

    return result


# ---------------------------------------------------------------------------
# 迁移器
# ---------------------------------------------------------------------------


class Migrator:
    def __init__(self, args: argparse.Namespace) -> None:
        self.args = args
        self.data_dir: Path = args.data_dir.resolve()
        self.official_data: Path = self._resolve_official_data(args.official_data)
        self.db_path: Path | None = args.db.resolve() if args.db else self._default_db_path()
        self.out_data: Path = (args.out_data or (self.data_dir.parent / "data_v2")).resolve()
        self.out_db: Path = (
            args.out_db.resolve()
            if args.out_db
            else (self.data_dir.parent / "db_v2" / "states.db").resolve()
        )
        self.report_path: Path | None = (
            args.report.resolve()
            if args.report
            else (self.out_data / "migration-report.json").resolve()
        )

        self.warnings: list[str] = []
        self.stats = MigrationStats()
        self.tag_map: dict[tuple[str, str], tuple[str, str]] = dict(DEFAULT_TAG_MAP)
        self.report = MigrationReport(
            data_dir=str(self.data_dir),
            db_path=str(self.db_path) if self.db_path else None,
            official_data=str(self.official_data),
            out_data=str(self.out_data),
            out_db=str(self.out_db) if self.out_db else None,
            dry_run=bool(args.dry_run),
        )

        self.v1_cards: list[tuple[Path, dict[str, Any]]] = []
        self.v1_decks: list[tuple[Path, dict[str, Any]]] = []
        self.v1_logics: list[tuple[Path, dict[str, Any]]] = []
        self.v1_banners: list[tuple[Path, dict[str, Any]]] = []

        self.v2_cards: list[tuple[Path, dict[str, Any]]] = []
        self.v2_decks: list[tuple[Path, dict[str, Any]]] = []
        self.v2_logics: list[tuple[Path, dict[str, Any]]] = []
        self.v2_banners: list[tuple[Path, dict[str, Any]]] = []

        self.official_card_ids: set[str] = set()
        self.official_deck_ids: set[str] = set()
        self.official_logic_ids: set[str] = set()
        self.official_banner_ids: set[str] = set()

        self.card_map: dict[int, str] = {}
        self.deck_map: dict[int, str] = {}
        self.logic_map: dict[int, str] = {}
        self.banner_map: dict[int, str] = {}

        self.local_cards: list[dict[str, Any]] = []
        self.local_decks: list[dict[str, Any]] = []
        self.local_logics: list[dict[str, Any]] = []
        self.local_banners: list[dict[str, Any]] = []

        # v0.1.0 / v0.1.1 把卡池状态内联在 Banner.logic_instance.state 中
        self.inline_states: dict[int, dict[str, Any]] = {}

        self.forked_sources: dict[str, dict[str, str]] = {
            kind: {} for kind in OBJECT_KINDS
        }

        self.v1_manifest_path: Path = (
            args.v1_official_manifest.resolve()
            if args.v1_official_manifest
            else (Path(__file__).resolve().parent / "v1_official_manifest.json")
        )
        self.v1_manifest = load_v1_manifest(
            self.v1_manifest_path,
            self.warnings,
            enabled=not args.no_v1_manifest,
        )

        self.official_card_index: dict[tuple[str | None, str | None], list[dict[str, Any]]] = defaultdict(list)
        self.official_deck_index: dict[str | None, list[dict[str, Any]]] = defaultdict(list)
        self.official_logic_index: dict[str | None, list[dict[str, Any]]] = defaultdict(list)
        self.official_banner_index: dict[str | None, list[dict[str, Any]]] = defaultdict(list)
        self.official_by_id: dict[str, dict[str, dict[str, Any]]] = {
            "cards": {},
            "decks": {},
            "logics": {},
            "banners": {},
        }

    # -- 路径与输入 -------------------------------------------------------

    def _resolve_official_data(self, value: Path | None) -> Path:
        candidate = value
        if candidate is None:
            candidate = Path(__file__).resolve().parent.parent / "data" / "official"
        candidate = candidate.resolve()
        if (candidate / "official").is_dir():
            candidate = (candidate / "official").resolve()
        if not (candidate / "cards").is_dir():
            raise MigrationFatal(
                f"找不到 v2 官方数据目录: {candidate}. "
                "请用 --official-data 指向 v2 应用的 data 目录或 data/official."
            )

        card_files = sorted((candidate / "cards").rglob("*.json"))
        if not card_files:
            raise MigrationFatal(f"v2 官方数据目录中没有卡片: {candidate}")
        sample = read_json(card_files[0])
        if not isinstance(sample, dict) or "global_id" not in sample:
            raise MigrationFatal(
                f"{candidate} 看起来不是 v2 官方数据 (卡片缺少 global_id); "
                "请传入 v2 应用的 data/official"
            )
        return candidate

    def _default_db_path(self) -> Path | None:
        candidate = self.data_dir.parent / "db" / "states.db"
        return candidate.resolve() if candidate.is_file() else None

    def warn(self, message: str) -> None:
        self.warnings.append(message)

    def check_inputs(self) -> None:
        if not self.data_dir.is_dir():
            raise MigrationFatal(f"数据目录不存在: {self.data_dir}")
        if not (self.data_dir / "cards").is_dir():
            raise MigrationFatal(f"{self.data_dir} 看起来不是 v1 数据目录 (缺少 cards/)")

        version_file = self.data_dir / "version.json"
        if version_file.is_file():
            version = read_json(version_file)
            if isinstance(version, dict) and int(version.get("format_version", 0)) >= DATA_FORMAT_VERSION:
                raise MigrationFatal(
                    f"{self.data_dir} 已经是数据格式 v{version.get('format_version')}, 不需要迁移"
                )
        if (self.data_dir / "official").is_dir():
            raise MigrationFatal(f"{self.data_dir} 含有 official/ 目录, 看起来已经是 v2 布局")

        self._check_output_path(self.out_data, "输出数据目录")
        if (
            self.out_data == self.official_data
            or self.official_data in self.out_data.parents
            or self.out_data in self.official_data.parents
        ):
            raise MigrationFatal("输出数据目录不能与 v2 官方数据目录重叠")
        if self.out_db == self.db_path:
            raise MigrationFatal("输出数据库不能与输入数据库相同")
        if self.out_db.exists():
            if self.args.dry_run:
                self.warn(f"输出数据库已存在, 实际运行需要使用 --force: {self.out_db}")
            elif not self.args.force:
                raise MigrationFatal(f"输出数据库已存在: {self.out_db} (使用 --force 覆盖)")

        if self.db_path and not self.db_path.is_file():
            self.warn(f"未找到 v1 状态数据库 {self.db_path}, 将创建空的 v2 数据库")
            self.db_path = None

    def _check_output_path(self, path: Path, label: str) -> None:
        if self.args.dry_run:
            if path.exists():
                self.warn(f"{label}已存在, 实际运行需要使用 --force: {path}")
            return

        try:
            if path == self.data_dir or self.data_dir in path.parents or path in self.data_dir.parents:
                raise MigrationFatal(f"{label}不能与输入数据目录重叠: {path}")
        except ValueError:
            pass

        if not path.exists():
            return
        if not self.args.force:
            if path.is_dir() and any(path.iterdir()):
                raise MigrationFatal(f"{label}已存在且非空: {path} (使用 --force 覆盖)")
            if path.is_file():
                raise MigrationFatal(f"{label}已存在: {path} (使用 --force 覆盖)")
        else:
            if path.is_dir():
                shutil.rmtree(path)
            else:
                path.unlink()

    def load_inputs(self) -> None:
        self.v1_cards = load_objects(self.data_dir / "cards")
        self.v1_decks = load_objects(self.data_dir / "decks")
        self.v1_logics = load_objects(self.data_dir / "logics")
        self.v1_banners = load_objects(self.data_dir / "banners")

        self.v2_cards = load_objects(self.official_data / "cards")
        self.v2_decks = load_objects(self.official_data / "decks")
        self.v2_logics = load_objects(self.official_data / "logics")
        self.v2_banners = load_objects(self.official_data / "banners")

        self._check_duplicate_ids(self.v1_cards, "Card")
        self._check_duplicate_ids(self.v1_decks, "Deck")
        self._check_duplicate_ids(self.v1_logics, "Logic")
        self._check_duplicate_ids(self.v1_banners, "Banner")

        for _, card in self.v2_cards:
            self.official_card_ids.add(str(card["global_id"]))
        for _, deck in self.v2_decks:
            self.official_deck_ids.add(str(deck["global_id"]))
        for _, logic in self.v2_logics:
            self.official_logic_ids.add(str(logic["global_id"]))
        for _, banner in self.v2_banners:
            self.official_banner_ids.add(str(banner["global_id"]))

        self._build_official_indexes()

    def _build_official_indexes(self) -> None:
        for _, card in self.v2_cards:
            self.official_card_index[
                (zh_text(card.get("content")), game_value(card.get("tags", [])))
            ].append(card)
            self.official_by_id["cards"][str(card["global_id"])] = card
        for kind, objects, index in (
            ("decks", self.v2_decks, self.official_deck_index),
            ("logics", self.v2_logics, self.official_logic_index),
            ("banners", self.v2_banners, self.official_banner_index),
        ):
            for _, obj in objects:
                index[zh_text(obj.get("name"))].append(obj)
                self.official_by_id[kind][str(obj["global_id"])] = obj

    def _check_duplicate_ids(self, objects: list[tuple[Path, dict[str, Any]]], label: str) -> None:
        seen: dict[int, Path] = {}
        for path, obj in objects:
            raw_id = obj.get("id")
            try:
                old_id = int(raw_id)
            except (TypeError, ValueError):
                raise MigrationFatal(f"{path}: {label} 的 id 不是整数: {raw_id!r}")
            if old_id in seen:
                raise MigrationFatal(
                    f"{label} Id {old_id} 重复: {seen[old_id]} 与 {path}"
                )
            seen[old_id] = path

    # -- 标签映射 ---------------------------------------------------------

    def build_tag_map(self) -> None:
        derived = derive_tag_map(
            self.v1_cards,
            self.v1_decks,
            self.v1_banners,
            self.v1_logics,
            self.v2_cards,
            self.v2_decks,
            self.v2_banners,
            self.v2_logics,
            self.warnings,
        )
        self.tag_map = derived
        if self.args.tag_map:
            self.tag_map.update(load_tag_map_override(self.args.tag_map, self.warnings))
        self.report.tag_map = {
            f"{source[0]}:{source[1]}": f"{target[0]}:{target[1]}"
            for source, target in sorted(self.tag_map.items())
        }

    # -- 对象迁移 ---------------------------------------------------------

    def v1_official_entry(
        self, kind: str, old_id: int, obj: dict[str, Any]
    ) -> dict[str, Any] | None:
        """若对象与某个 v1 版本的官方默认对象完全一致, 返回对应清单条目."""
        entries = self.v1_manifest.get(kind, {}).get(str(old_id), [])
        if not entries:
            return None
        try:
            digest = v1_signature_hash(kind, obj)
        except (KeyError, TypeError, ValueError):
            return None
        for entry in entries:
            if entry.get("hash") == digest:
                return entry
        return None

    def _official_default_counterpart(
        self,
        kind: str,
        entry: dict[str, Any] | None,
        raw_obj: dict[str, Any],
    ) -> dict[str, Any] | None:
        """为 v1 官方默认对象找到 v2 官方对应对象."""
        official_id = entry.get("global_id") if entry else None
        if official_id:
            found = self.official_by_id.get(kind, {}).get(str(official_id))
            if found is not None:
                return found
            self.warn(
                f"{kind} 的 v1 官方清单指向 {official_id}, 但当前 v2 官方数据中不存在; "
                "尝试按名称/内容匹配"
            )

        if kind == "cards":
            candidates = self.official_card_index.get(
                (zh_text(raw_obj.get("content")), game_value(raw_obj.get("tags", []))),
                [],
            )
        elif kind == "decks":
            candidates = self.official_deck_index.get(zh_text(raw_obj.get("name")), [])
        elif kind == "logics":
            candidates = self.official_logic_index.get(zh_text(raw_obj.get("name")), [])
        elif kind == "banners":
            candidates = self.official_banner_index.get(zh_text(raw_obj.get("name")), [])
        else:
            candidates = []

        if len(candidates) == 1:
            return candidates[0]
        if len(candidates) > 1:
            self.warn(
                f"{kind} 的官方对应对象不唯一 ({len(candidates)} 个), "
                "无法确定 v1 官方默认对象"
            )
        return None

    def _record_inline_state(self, old_id: int, state: Any, path: Path) -> None:
        """记录 v0.1.0/v0.1.1 卡池内联状态, 稍后写入 v2 状态库."""
        if state is None:
            return
        if not isinstance(state, dict):
            self.warn(f"{path}: logic_instance.state 不是对象, 已忽略")
            return
        try:
            total_counter = int(state.get("total_counter", 0) or 0)
        except (TypeError, ValueError):
            self.warn(f"{path}: total_counter 不是整数, 按 0 处理")
            total_counter = 0
        logic_state = {
            key: value for key, value in state.items() if key != "total_counter"
        }
        self.inline_states[old_id] = {
            "total_counter": total_counter,
            "logic_state": json.dumps(logic_state, ensure_ascii=False, separators=(",", ":")),
        }

    def _record(
        self,
        kind: str,
        old_id: int,
        global_id: str | None,
        status: str,
        name: str | None,
        forked_from: str | None = None,
        reason: str | None = None,
    ) -> None:
        entry: dict[str, Any] = {
            "old_id": old_id,
            "global_id": global_id,
            "status": status,
            "name": name,
        }
        if forked_from:
            entry["forked_from"] = forked_from
        if reason:
            entry["reason"] = reason
        self.report.objects[kind].append(entry)

        stats = self.stats.kinds[kind]
        stats.read += 1
        if status == "official":
            stats.official += 1
        elif status == "fork":
            stats.fork += 1
        elif status == "local":
            stats.local += 1
        elif status == "skipped":
            stats.skipped += 1

    def _fork_or_local(
        self,
        kind: str,
        official_id: str,
        new_id: str,
        name: str,
    ) -> tuple[str, str | None]:
        existing = self.forked_sources[kind].get(official_id)
        if existing is not None:
            self.warn(
                f"{kind} {name!r} 与官方对象 {official_id} 不一致, "
                f"但该官方对象已有派生 {existing}, 改为独立本地对象"
            )
            return "local", None
        self.forked_sources[kind][official_id] = new_id
        return "fork", official_id

    def migrate_cards(self) -> None:
        index: dict[tuple[str | None, str | None], list[dict[str, Any]]] = defaultdict(list)
        for _, card in self.v2_cards:
            index[(zh_text(card.get("content")), game_value(card.get("tags", [])))].append(card)

        for path, card in self.v1_cards:
            old_id = int(card["id"])
            content = to_localized(card.get("content"))
            if not content:
                self.warn(f"{path}: 卡片内容为空, 已跳过")
                self._record("cards", old_id, None, "skipped", None, reason="empty content")
                continue

            official_default = self.v1_official_entry("cards", old_id, card)
            default_counterpart = None
            if official_default is not None:
                default_counterpart = self._official_default_counterpart(
                    "cards", official_default, card
                )
                if default_counterpart is None:
                    self.warn(
                        f"Card {old_id} 是 v1 官方默认对象, 但 v2 官方数据中找不到对应对象, "
                        "将保留为本地对象"
                    )
            if default_counterpart is not None:
                official_id = str(default_counterpart["global_id"])
                self.card_map[old_id] = official_id
                self._record("cards", old_id, official_id, "official", zh_text(content))
                continue

            tags = map_tags(card.get("tags", []), self.tag_map, self.warnings, f"Card {old_id}")
            mapped_game = next(
                (item["value"] for item in tags if item["namespace"] == "game"), None
            )
            candidates = (
                []
                if official_default is not None
                else index.get((zh_text(content), mapped_game), [])
            )

            signature = (zh_text(content), tag_signature(tags))
            exact = [item for item in candidates if (zh_text(item.get("content")), tag_signature(item.get("tags", []))) == signature]

            if len(exact) == 1:
                official_id = str(exact[0]["global_id"])
                self.card_map[old_id] = official_id
                self._record("cards", old_id, official_id, "official", zh_text(content))
                continue

            new_id = uuid7()
            forked_from: str | None = None
            status = "local"
            if len(candidates) == 1:
                official_id = str(candidates[0]["global_id"])
                status, forked_from = self._fork_or_local(
                    "cards", official_id, new_id, zh_text(content) or ""
                )

            obj: dict[str, Any] = {
                "global_id": new_id,
                "origin": "local",
            }
            if forked_from:
                obj["forked_from"] = forked_from
            obj["content"] = content
            if tags:
                obj["tags"] = tags
            self.local_cards.append(obj)
            self.card_map[old_id] = new_id
            self._record("cards", old_id, new_id, status, zh_text(content), forked_from)

    def migrate_logics(self) -> None:
        index: dict[str | None, list[dict[str, Any]]] = defaultdict(list)
        for _, logic in self.v2_logics:
            index[zh_text(logic.get("name"))].append(logic)

        for path, logic in self.v1_logics:
            old_id = int(logic["id"])
            name = to_localized(logic.get("name"))
            if not name:
                self.warn(f"{path}: 逻辑名称为空, 已跳过")
                self._record("logics", old_id, None, "skipped", None, reason="empty name")
                continue

            variant = logic.get("variant")
            if not isinstance(variant, dict) or "variant" not in variant:
                self.warn(f"{path}: 逻辑缺少合法的 variant, 已跳过")
                self._record("logics", old_id, None, "skipped", zh_text(name), reason="invalid variant")
                continue

            official_default = self.v1_official_entry("logics", old_id, logic)
            default_counterpart = None
            if official_default is not None:
                default_counterpart = self._official_default_counterpart(
                    "logics", official_default, logic
                )
                if default_counterpart is None:
                    self.warn(
                        f"Logic {old_id} 是 v1 官方默认对象, 但 v2 官方数据中找不到对应对象, "
                        "将保留为本地对象"
                    )
            if default_counterpart is not None:
                official_id = str(default_counterpart["global_id"])
                self.logic_map[old_id] = official_id
                self._record("logics", old_id, official_id, "official", zh_text(name))
                continue

            tags = map_tags(logic.get("tags", []), self.tag_map, self.warnings, f"Logic {old_id}")
            signature = (zh_text(name), canonical_json(variant))
            candidates = [] if official_default is not None else index.get(zh_text(name), [])
            exact = [item for item in candidates if logic_signature(item) == signature]

            if len(exact) == 1:
                official_id = str(exact[0]["global_id"])
                self.logic_map[old_id] = official_id
                self._record("logics", old_id, official_id, "official", zh_text(name))
                continue

            new_id = uuid7()
            forked_from: str | None = None
            status = "local"
            if len(candidates) == 1:
                official_id = str(candidates[0]["global_id"])
                status, forked_from = self._fork_or_local(
                    "logics", official_id, new_id, zh_text(name) or ""
                )

            obj: dict[str, Any] = {
                "global_id": new_id,
                "origin": "local",
            }
            if forked_from:
                obj["forked_from"] = forked_from
            obj["name"] = name
            obj["variant"] = variant
            if tags:
                obj["tags"] = tags
            self.local_logics.append(obj)
            self.logic_map[old_id] = new_id
            self._record("logics", old_id, new_id, status, zh_text(name), forked_from)

    def migrate_decks(self) -> None:
        index: dict[str | None, list[dict[str, Any]]] = defaultdict(list)
        for _, deck in self.v2_decks:
            index[zh_text(deck.get("name"))].append(deck)

        for path, deck in self.v1_decks:
            old_id = int(deck["id"])
            name = to_localized(deck.get("name"))
            if not name:
                self.warn(f"{path}: 卡组名称为空, 已跳过")
                self._record("decks", old_id, None, "skipped", None, reason="empty name")
                continue

            official_default = self.v1_official_entry("decks", old_id, deck)
            default_counterpart = None
            if official_default is not None:
                default_counterpart = self._official_default_counterpart(
                    "decks", official_default, deck
                )
                if default_counterpart is None:
                    self.warn(
                        f"Deck {old_id} 是 v1 官方默认对象, 但 v2 官方数据中找不到对应对象, "
                        "将保留为本地对象"
                    )
            if default_counterpart is not None:
                official_id = str(default_counterpart["global_id"])
                self.deck_map[old_id] = official_id
                self._record("decks", old_id, official_id, "official", zh_text(name))
                continue

            tags = map_tags(deck.get("tags", []), self.tag_map, self.warnings, f"Deck {old_id}")
            raw_members = deck.get("members")
            if isinstance(raw_members, list):
                # v0.1.0 / v0.1.1: 静态成员 Id 列表
                converted_members = {
                    "conditions": [
                        convert_condition(
                            {"type": "include_ids", "ids": raw_members},
                            self.card_map,
                            self.tag_map,
                            self.warnings,
                            f"Deck {old_id}.members",
                        )
                    ]
                }
            elif isinstance(raw_members, dict):
                converted_members = {
                    "conditions": convert_conditions(
                        raw_members.get("conditions", []) or [],
                        self.card_map,
                        self.tag_map,
                        self.warnings,
                        f"Deck {old_id}.members",
                    )
                }
            else:
                converted_members = {"conditions": []}

            event_groups: dict[str, dict[str, Any]] = {}
            for key, group in (deck.get("event_groups", {}) or {}).items():
                new_key = map_event_key(key, self.tag_map, self.warnings, f"Deck {old_id}.event_groups")
                if new_key in event_groups:
                    self.warn(f"Deck {old_id}: 活动组键 {key!r} 映射后与 {new_key!r} 冲突, 已合并")
                if isinstance(group, list):
                    conditions = [
                        convert_condition(
                            {"type": "include_ids", "ids": group},
                            self.card_map,
                            self.tag_map,
                            self.warnings,
                            f"Deck {old_id}.event_groups[{new_key}]",
                        )
                    ]
                elif isinstance(group, dict):
                    conditions = convert_conditions(
                        group.get("conditions", []) or [],
                        self.card_map,
                        self.tag_map,
                        self.warnings,
                        f"Deck {old_id}.event_groups[{new_key}]",
                    )
                else:
                    conditions = []
                event_groups[new_key] = {"conditions": conditions}

            signature = (
                zh_text(name),
                tag_signature(tags),
                tuple(condition_signature(item) for item in converted_members["conditions"]),
                tuple(
                    sorted(
                        (key, tuple(condition_signature(item) for item in group["conditions"]))
                        for key, group in event_groups.items()
                    )
                ),
            )
            candidates = [] if official_default is not None else index.get(zh_text(name), [])
            exact = [item for item in candidates if deck_signature(item) == signature]

            if len(exact) == 1:
                official_id = str(exact[0]["global_id"])
                self.deck_map[old_id] = official_id
                self._record("decks", old_id, official_id, "official", zh_text(name))
                continue

            new_id = uuid7()
            forked_from: str | None = None
            status = "local"
            if len(candidates) == 1:
                official_id = str(candidates[0]["global_id"])
                status, forked_from = self._fork_or_local(
                    "decks", official_id, new_id, zh_text(name) or ""
                )

            obj: dict[str, Any] = {
                "global_id": new_id,
                "origin": "local",
            }
            if forked_from:
                obj["forked_from"] = forked_from
            obj["name"] = name
            obj["members"] = converted_members
            obj["event_groups"] = event_groups
            if tags:
                obj["tags"] = tags
            self.local_decks.append(obj)
            self.deck_map[old_id] = new_id
            self._record("decks", old_id, new_id, status, zh_text(name), forked_from)

    def migrate_banners(self) -> None:
        index: dict[str | None, list[dict[str, Any]]] = defaultdict(list)
        for _, banner in self.v2_banners:
            index[zh_text(banner.get("name"))].append(banner)

        for path, banner in self.v1_banners:
            old_id = int(banner["id"])
            name = to_localized(banner.get("name"))
            if not name:
                self.warn(f"{path}: 卡池名称为空, 已跳过")
                self._record("banners", old_id, None, "skipped", None, reason="empty name")
                continue

            # v0.1.0 / v0.1.1: logic_id 与运行时状态内联在 logic_instance 中
            raw_logic_id = banner.get("logic_id")
            inline_state = None
            if raw_logic_id is None:
                instance = banner.get("logic_instance") or {}
                raw_logic_id = instance.get("logic_id")
                inline_state = instance.get("state")
            if inline_state is not None:
                self._record_inline_state(old_id, inline_state, path)

            official_default = self.v1_official_entry("banners", old_id, banner)
            default_counterpart = None
            if official_default is not None:
                default_counterpart = self._official_default_counterpart(
                    "banners", official_default, banner
                )
                if default_counterpart is None:
                    self.warn(
                        f"Banner {old_id} 是 v1 官方默认对象, 但 v2 官方数据中找不到对应对象, "
                        "将保留为本地对象"
                    )
            if default_counterpart is not None:
                official_id = str(default_counterpart["global_id"])
                self.banner_map[old_id] = official_id
                self._record("banners", old_id, official_id, "official", zh_text(name))
                continue

            try:
                old_deck_id = int(banner["deck_id"])
                old_logic_id = int(raw_logic_id)
            except (KeyError, TypeError, ValueError):
                self.warn(f"{path}: 卡池缺少合法的 deck_id / logic_id, 已跳过")
                self._record("banners", old_id, None, "skipped", zh_text(name), reason="missing refs")
                continue

            deck_id = self.deck_map.get(old_deck_id)
            logic_id = self.logic_map.get(old_logic_id)
            if not deck_id or not logic_id:
                missing = []
                if not deck_id:
                    missing.append(f"Deck {old_deck_id}")
                if not logic_id:
                    missing.append(f"Logic {old_logic_id}")
                self.warn(f"{path}: 引用的 {', '.join(missing)} 不存在, 已跳过该卡池")
                self._record("banners", old_id, None, "skipped", zh_text(name), reason="missing refs")
                continue

            tags = map_tags(banner.get("tags", []), self.tag_map, self.warnings, f"Banner {old_id}")
            converted = {
                "name": name,
                "tags": tags,
                "deck_id": deck_id,
                "logic_id": logic_id,
                "state_scope": "banner",
            }
            signature = (
                zh_text(name),
                tag_signature(tags),
                str(deck_id),
                str(logic_id),
                "banner",
            )
            candidates = [] if official_default is not None else index.get(zh_text(name), [])
            exact = [item for item in candidates if banner_signature(item) == signature]

            if len(exact) == 1:
                official_id = str(exact[0]["global_id"])
                self.banner_map[old_id] = official_id
                self._record("banners", old_id, official_id, "official", zh_text(name))
                continue

            new_id = uuid7()
            forked_from: str | None = None
            status = "local"
            if len(candidates) == 1:
                official_id = str(candidates[0]["global_id"])
                status, forked_from = self._fork_or_local(
                    "banners", official_id, new_id, zh_text(name) or ""
                )

            obj: dict[str, Any] = {
                "global_id": new_id,
                "origin": "local",
            }
            if forked_from:
                obj["forked_from"] = forked_from
            obj.update(converted)
            if tags:
                obj["tags"] = tags
            self.local_banners.append(obj)
            self.banner_map[old_id] = new_id
            self._record("banners", old_id, new_id, status, zh_text(name), forked_from)

    # -- 输出 -------------------------------------------------------------

    def write_output(self) -> None:
        self.out_data.mkdir(parents=True, exist_ok=True)
        shutil.copytree(self.official_data, self.out_data / "official")
        for kind in OBJECT_KINDS:
            (self.out_data / "local" / kind).mkdir(parents=True, exist_ok=True)
        (self.out_data / "packs").mkdir(parents=True, exist_ok=True)
        write_json(self.out_data / "version.json", {"format_version": DATA_FORMAT_VERSION})

        for kind, objects in (
            ("cards", self.local_cards),
            ("decks", self.local_decks),
            ("logics", self.local_logics),
            ("banners", self.local_banners),
        ):
            for obj in objects:
                write_json(self.out_data / "local" / kind / f"{obj['global_id']}.json", obj)

        assets = self.data_dir / "assets"
        if assets.is_dir():
            shutil.copytree(assets, self.out_data / "local" / "assets")
            self.warn("已复制 v1 assets/ 到 v2 的 local/assets/, 但 v1 卡片没有资产引用")

    def write_report(self) -> None:
        if self.report_path:
            write_json(self.report_path, self.report.as_dict())

    def migrate_states(self) -> None:
        if not self.out_db:
            return
        self.out_db.parent.mkdir(parents=True, exist_ok=True)
        if self.out_db.exists():
            self.out_db.unlink()

        out_conn = sqlite3.connect(str(self.out_db))
        try:
            out_conn.execute(
                """
                CREATE TABLE IF NOT EXISTS instance_states (
                    scope_key       TEXT NOT NULL,
                    user_id         INTEGER NOT NULL DEFAULT 0,
                    total_counter   INTEGER NOT NULL DEFAULT 0,
                    logic_state     TEXT,
                    PRIMARY KEY     (scope_key)
                )
                """
            )

            db_banners: set[int] = set()

            if self.db_path is not None:
                uri = self.db_path.resolve().as_uri() + "?mode=ro"
                in_conn = sqlite3.connect(uri, uri=True)
                try:
                    table = in_conn.execute(
                        "SELECT name FROM sqlite_master WHERE type='table' AND name='instance_states'"
                    ).fetchone()
                    if table is None:
                        self.warn(
                            f"{self.db_path} 中没有 instance_states 表, "
                            "将只迁移卡池内联状态"
                        )
                    else:
                        columns = [row[1] for row in in_conn.execute("PRAGMA table_info(instance_states)")]
                        if "scope_key" in columns:
                            raise MigrationFatal(
                                f"{self.db_path} 已经是 v2 结构 (存在 scope_key), 不需要迁移"
                            )
                        if "banner_id" not in columns:
                            raise MigrationFatal(
                                f"{self.db_path} 的 instance_states 表缺少 banner_id 列, 无法识别为 v1"
                            )

                        rows = in_conn.execute(
                            "SELECT user_id, banner_id, total_counter, logic_state FROM instance_states"
                        ).fetchall()
                        for user_id, banner_id, total_counter, logic_state in rows:
                            try:
                                user_id_int = int(user_id)
                                banner_id_int = int(banner_id)
                                total_int = int(total_counter or 0)
                            except (TypeError, ValueError):
                                self.warn(
                                    f"跳过无法解析的状态行: user_id={user_id!r}, banner_id={banner_id!r}"
                                )
                                self.stats.states_skipped += 1
                                continue

                            if user_id_int != 0:
                                self.warn(
                                    f"跳过非 0 用户的状态行 (user_id={user_id_int}, banner_id={banner_id_int}); "
                                    "v2 目前是单用户模式"
                                )
                                self.stats.states_skipped += 1
                                continue

                            mapped_banner = self.banner_map.get(banner_id_int)
                            if not mapped_banner:
                                self.warn(
                                    f"跳过 Banner Id {banner_id_int} 的状态: 该卡池未成功迁移"
                                )
                                self.stats.states_skipped += 1
                                continue

                            state_text = logic_state
                            if state_text is None or state_text == "":
                                state_text = "null"
                            else:
                                try:
                                    json.loads(state_text)
                                except (TypeError, json.JSONDecodeError):
                                    self.warn(
                                        f"Banner Id {banner_id_int} 的 logic_state 不是合法 JSON, 已替换为 null"
                                    )
                                    state_text = "null"

                            out_conn.execute(
                                "INSERT OR REPLACE INTO instance_states "
                                "(scope_key, user_id, total_counter, logic_state) VALUES (?, ?, ?, ?)",
                                (f"banner:{mapped_banner}", 0, total_int, state_text),
                            )
                            db_banners.add(banner_id_int)
                            self.stats.states_migrated += 1
                finally:
                    in_conn.close()

            # v0.1.0 / v0.1.1: 卡池状态内联在 Banner.logic_instance.state 中
            for old_id, state in self.inline_states.items():
                if old_id in db_banners:
                    self.warn(
                        f"Banner Id {old_id} 同时存在数据库状态与内联状态, 使用数据库状态"
                    )
                    continue
                mapped_banner = self.banner_map.get(old_id)
                if not mapped_banner:
                    self.warn(f"跳过 Banner Id {old_id} 的内联状态: 该卡池未成功迁移")
                    self.stats.states_skipped += 1
                    continue
                out_conn.execute(
                    "INSERT OR REPLACE INTO instance_states "
                    "(scope_key, user_id, total_counter, logic_state) VALUES (?, ?, ?, ?)",
                    (
                        f"banner:{mapped_banner}",
                        0,
                        int(state["total_counter"]),
                        state["logic_state"],
                    ),
                )
                self.stats.states_migrated += 1

            out_conn.commit()
        finally:
            out_conn.close()

    # -- 运行 -------------------------------------------------------------

    def run(self) -> MigrationReport:
        self.check_inputs()
        self.load_inputs()
        self.build_tag_map()
        self.migrate_cards()
        self.migrate_logics()
        self.migrate_decks()
        self.migrate_banners()

        self.report.stats = self.stats
        self.report.warnings = self.warnings

        if self.args.dry_run:
            return self.report

        self.write_output()
        self.migrate_states()
        self.report.stats = self.stats
        self.write_report()
        return self.report


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def parse_args(argv: Iterable[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="将 Wishes v1 数据目录与状态数据库迁移为 v2 格式",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "示例:\n"
            "  python scripts/migrate_v1_to_v2.py \\\n"
            "    --data-dir data --db db/states.db \\\n"
            "    --official-data data/official \\\n"
            "    --out-data data_v2 --out-db db_v2/states.db\n"
        ),
    )
    parser.add_argument("--data-dir", type=Path, required=True, help="v1 数据目录")
    parser.add_argument("--db", type=Path, help="v1 状态数据库 (默认: <data-dir>/../db/states.db)")
    parser.add_argument(
        "--official-data",
        type=Path,
        help="v2 官方数据目录或数据根目录 (默认: 仓库根目录的 data/official)",
    )
    parser.add_argument("--out-data", type=Path, help="输出 v2 数据目录 (默认: <data-dir>/../data_v2)")
    parser.add_argument("--out-db", type=Path, help="输出 v2 状态数据库 (默认: <data-dir>/../db_v2/states.db)")
    parser.add_argument("--tag-map", type=Path, help="额外的标签映射 JSON")
    parser.add_argument(
        "--v1-official-manifest",
        type=Path,
        help="v1 官方默认对象清单 (默认: scripts/v1_official_manifest.json)",
    )
    parser.add_argument(
        "--no-v1-manifest",
        action="store_true",
        help="忽略 v1 官方清单, 与 v2 官方数据不同的对象一律保留为派生",
    )
    parser.add_argument("--report", type=Path, help="迁移报告路径 (默认: <out-data>/migration-report.json)")
    parser.add_argument("--dry-run", action="store_true", help="只分析并打印摘要, 不写任何文件")
    parser.add_argument("--force", action="store_true", help="允许覆盖已存在的输出目录/数据库")
    parser.add_argument("--strict", action="store_true", help="存在警告时返回非零退出码")
    return parser.parse_args(argv)


def print_summary(migrator: Migrator, report: MigrationReport) -> None:
    print("=== Wishes v1 -> v2 迁移 ===")
    print(f"源数据目录: {report.data_dir}")
    print(f"官方参考:   {report.official_data}")
    print(f"输出数据:   {report.out_data}")
    print(f"输出数据库: {report.out_db or '(不迁移)'}")
    print(f"标签映射:   {len(report.tag_map)} 条")
    manifest_note = (
        "已禁用"
        if migrator.args.no_v1_manifest
        else f"{sum(len(v) for v in migrator.v1_manifest.values())} 个对象"
    )
    print(f"v1 官方清单: {migrator.v1_manifest_path} ({manifest_note})")
    print()
    for kind in OBJECT_KINDS:
        stats = report.stats.kinds[kind]
        print(
            f"{kind:8s} 读取 {stats.read:4d} | 官方 {stats.official:4d} | "
            f"派生 {stats.fork:4d} | 本地 {stats.local:4d} | 跳过 {stats.skipped:4d}"
        )
    print()
    print(f"状态行: 迁移 {report.stats.states_migrated}, 跳过 {report.stats.states_skipped}")
    if report.warnings:
        print()
        print(f"警告 ({len(report.warnings)} 条):")
        for warning in report.warnings[:50]:
            print(f"  - {warning}")
        if len(report.warnings) > 50:
            print(f"  ... 其余 {len(report.warnings) - 50} 条见迁移报告")
    if report.dry_run:
        print()
        print("dry-run: 未写入任何文件")


def main(argv: Iterable[str] | None = None) -> int:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
        sys.stderr.reconfigure(encoding="utf-8", errors="replace")

    args = parse_args(argv)
    try:
        migrator = Migrator(args)
        report = migrator.run()
    except MigrationFatal as exc:
        print(f"错误: {exc}", file=sys.stderr)
        return 1

    print_summary(migrator, report)
    if args.strict and report.warnings:
        print(f"严格模式: 存在 {len(report.warnings)} 条警告", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
