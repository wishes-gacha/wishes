//! # 引用覆盖层数据库
//!
//! 官方数据与拓展包数据在运行时永远只读. 用户编辑只读对象时产生的
//! **引用重定向** (例如: 派生卡片遮蔽原卡片后, 卡组中对原卡片的引用
//! 应解析为派生卡片) 不会写回任何 JSON 文件, 而是存放在这里的本地
//! 覆盖层.
//!
//! 所有引用覆盖共用一个数据库文件 (默认 `db/overrides.db`),
//! 按对象类型分表:
//!
//! - `card_ref_overrides`: 卡片引用覆盖 (当前启用).
//! - `deck_ref_overrides`: 卡池对卡组的引用覆盖 (预留, 随 Banner CRUD 启用).
//! - `logic_ref_overrides`: 卡池对逻辑的引用覆盖 (预留, 随 Logic CRUD 启用).
//!
//! 每张表保存 `from_id -> to_id` 的映射: 任何引用 `from_id` 的位置
//! 在解析时都应视为引用 `to_id`.
//!
//! # 与状态数据库的关系
//!
//! 本模块与 `state_repo` (卡池运行时状态) 相互独立, 使用不同的数据库文件,
//! 互不影响.

use std::path::Path;
use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use crate::domain::ids::GlobalId;

/// 引用覆盖的对象类型.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    /// 卡片引用 (`Deck` 的 `include_ids` / `exclude_ids`).
    Card,

    /// 卡组引用 (`Banner.deck_id`). 预留, 随 Banner CRUD 启用.
    Deck,

    /// 逻辑引用 (`Banner.logic_id`). 预留, 随 Logic CRUD 启用.
    Logic,
}

impl RefKind {
    /// 该类型对应的表名.
    fn table(self) -> &'static str {
        match self {
            RefKind::Card => "card_ref_overrides",
            RefKind::Deck => "deck_ref_overrides",
            RefKind::Logic => "logic_ref_overrides",
        }
    }
}

/// 引用覆盖仓库.
///
/// 包含一个 SQLite 连接, 内部使用 `Mutex` 保证线程安全.
///
/// `Mutex` 由 `parking_lot` crate 提供.
pub struct OverlayRepository {
    conn: Mutex<Connection>,
}

impl OverlayRepository {
    /// 打开或创建数据库, 并初始化三类覆盖表.
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)
            .with_context(|| format!(
                "打开引用覆盖数据库失败 - file: {}",
                db_path.display()
            ))?;

        Self::ensure_schema(&conn)?;

        Ok(Self { conn: Mutex::new(conn) })
    }

    /// 打开一个内存数据库, 用于测试或临时场景.
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::ensure_schema(&conn)?;
        Ok(Self { conn: Mutex::new(conn) })
    }

    /// 确保三类覆盖表存在.
    fn ensure_schema(conn: &Connection) -> Result<()> {
        for kind in [RefKind::Card, RefKind::Deck, RefKind::Logic] {
            conn.execute(
                &format!(
                    r#"
                        CREATE TABLE IF NOT EXISTS {} (
                            from_id TEXT NOT NULL PRIMARY KEY,
                            to_id   TEXT NOT NULL
                        )
                    "#,
                    kind.table()
                ),
                [],
            )
            .with_context(|| format!("初始化引用覆盖表 {} 失败", kind.table()))?;
        }

        Ok(())
    }

    /// 读取某一类的全部覆盖记录.
    ///
    /// 无法解析为 `GlobalId` 的记录会被记录警告并跳过, 不阻断启动.
    pub fn load(&self, kind: RefKind) -> Result<Vec<(GlobalId, GlobalId)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(&format!(
            "SELECT from_id, to_id FROM {}",
            kind.table()
        ))?;
        let mut rows = stmt.query([])?;

        let mut entries = Vec::new();
        while let Some(row) = rows.next()? {
            let from_id: String = row.get(0)?;
            let to_id: String = row.get(1)?;
            match (GlobalId::parse(&from_id), GlobalId::parse(&to_id)) {
                (Ok(from), Ok(to)) => entries.push((from, to)),
                _ => tracing::warn!(
                    table = kind.table(),
                    from_id = %from_id,
                    to_id = %to_id,
                    "引用覆盖记录中的 Id 无法解析, 已跳过"
                ),
            }
        }

        Ok(entries)
    }

    /// 写入一条覆盖记录 (已存在时更新目标).
    pub fn set(&self, kind: RefKind, from: GlobalId, to: GlobalId) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            &format!(
                "INSERT INTO {} (from_id, to_id) VALUES (?1, ?2)
                 ON CONFLICT(from_id) DO UPDATE SET to_id = excluded.to_id",
                kind.table()
            ),
            params![from.to_hyphenated(), to.to_hyphenated()],
        )?;
        Ok(())
    }

    /// 删除 `from` 对应的覆盖记录 (不存在时不做任何事).
    pub fn remove(&self, kind: RefKind, from: GlobalId) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            &format!("DELETE FROM {} WHERE from_id = ?1", kind.table()),
            params![from.to_hyphenated()],
        )?;
        Ok(())
    }

    /// 用给定记录**整体替换**某一类的覆盖表 (单事务).
    ///
    /// 用于启动时按分叉/遮蔽状态重建卡片覆盖表, 同时清除失效记录.
    pub fn replace_all(&self, kind: RefKind, entries: &[(GlobalId, GlobalId)]) -> Result<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;

        tx.execute(&format!("DELETE FROM {}", kind.table()), [])?;
        {
            let mut stmt = tx.prepare(&format!(
                "INSERT INTO {} (from_id, to_id) VALUES (?1, ?2)",
                kind.table()
            ))?;
            for (from, to) in entries {
                stmt.execute(params![from.to_hyphenated(), to.to_hyphenated()])?;
            }
        }
        tx.commit()?;

        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use uuid::Uuid;
    use super::*;

    fn temp_db_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("wishes-overrides-{}-{}.db", tag, Uuid::now_v7()))
    }

    fn pair() -> (GlobalId, GlobalId) {
        (GlobalId::new(), GlobalId::new())
    }

    #[test]
    fn set_and_load_are_per_kind() {
        let repo = OverlayRepository::open_in_memory().unwrap();
        let (card_from, card_to) = pair();
        let (deck_from, deck_to) = pair();
        let (logic_from, logic_to) = pair();

        repo.set(RefKind::Card, card_from, card_to).unwrap();
        repo.set(RefKind::Deck, deck_from, deck_to).unwrap();
        repo.set(RefKind::Logic, logic_from, logic_to).unwrap();

        assert_eq!(repo.load(RefKind::Card).unwrap(), vec![(card_from, card_to)]);
        assert_eq!(repo.load(RefKind::Deck).unwrap(), vec![(deck_from, deck_to)]);
        assert_eq!(repo.load(RefKind::Logic).unwrap(), vec![(logic_from, logic_to)]);

        repo.remove(RefKind::Card, card_from).unwrap();
        assert!(repo.load(RefKind::Card).unwrap().is_empty());
        assert_eq!(repo.load(RefKind::Deck).unwrap().len(), 1, "删除不应影响其他表");
    }

    #[test]
    fn set_updates_existing_record() {
        let repo = OverlayRepository::open_in_memory().unwrap();
        let (from, first) = pair();
        let second = GlobalId::new();

        repo.set(RefKind::Card, from, first).unwrap();
        repo.set(RefKind::Card, from, second).unwrap();

        assert_eq!(repo.load(RefKind::Card).unwrap(), vec![(from, second)]);
    }

    #[test]
    fn replace_all_prunes_stale_rows() {
        let repo = OverlayRepository::open_in_memory().unwrap();
        let (stale_from, stale_to) = pair();
        let (kept_from, kept_to) = pair();

        repo.set(RefKind::Card, stale_from, stale_to).unwrap();
        repo.replace_all(RefKind::Card, &[(kept_from, kept_to)]).unwrap();

        assert_eq!(repo.load(RefKind::Card).unwrap(), vec![(kept_from, kept_to)]);
    }

    #[test]
    fn file_database_persists_across_reopen() {
        let path = temp_db_path("persist");
        let (from, to) = pair();

        {
            let repo = OverlayRepository::new(&path).unwrap();
            repo.set(RefKind::Card, from, to).unwrap();
        }

        let reopened = OverlayRepository::new(&path).unwrap();
        assert_eq!(reopened.load(RefKind::Card).unwrap(), vec![(from, to)]);

        std::fs::remove_file(&path).ok();
    }
}
