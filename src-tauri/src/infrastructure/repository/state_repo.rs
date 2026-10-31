//! # 状态数据库
//! 
//! 将卡池的运行时状态 (`total_counter` + 逻辑实例状态 `serde_json::Value`)
//! 持久化到 SQLite 数据库.
//! 
//! # 数据格式 v2 的变化
//! 
//! 表的主键从 `banner_id` (整数) 改为 `scope_key` (**字符串**):
//! 
//! - `"banner:<uuid>"`: 该卡池独立维护状态 (默认).
//! - `"group:<name>"`: 同组的多个卡池**共享**同一份状态
//!   (如明日方舟标准池跨池继承保底).
//! 
//! 这样"状态属于哪个作用域"与"卡池本身"解耦, 卡池被替换/分叉时也能正确处理.
//! 
//! # 关于旧表
//! 
//! 本次不做用户数据迁移, 因此启动时若检测到 v1 时代的 `banner_id` 旧表,
//! 会直接重建 (旧的保底进度会丢失).

use std::path::Path;
use parking_lot::Mutex;
use anyhow::{Context, Result};
use rusqlite::{Connection, Error, ToSql, types::{ToSqlOutput, Value as SqlValue}};
use serde_json::Value as JsonValue;
use crate::domain::{ids::UserId, banner::BannerRuntimeState};

/// 为 `UserId` 实现 `ToSql`, 将其 `u64` 值转为 `i64` 以适配 SQLite 整数.
/// 
/// # 注意
/// 要求 `UserId.0` 不超过 `i64::MAX`, 否则转换将失败并返回 `rusqlite::Error::ToSqlConversionFailure`.
impl ToSql for UserId {                                                         // Id 的 u64 值转为 i64 再转 sql 数据类型
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {    // 需确保 Id 值不会超过 i64::MAX
        let id_i64 = i64::try_from(self.0)
            .map_err(|_| Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("User Id 转换至 sql 失败 (值 {} 超出 i64::MAX)", self.0)
            ))))?;
        Ok(ToSqlOutput::Owned(SqlValue::Integer(id_i64)))
    }
}

/// 状态数据库.
/// 
/// 包含一个 SQLite 连接, 内部使用 `Mutex` 保证线程安全.
/// 
/// `Mutex` 由 `parking_lot` crate 提供.
pub struct StateRepository {
    conn: Mutex<Connection>,
}

impl StateRepository {
    /// 打开或创建数据库, 并初始化 `instance_states` 表.
    /// 
    /// 若检测到 v1 时代旧表结构, 直接重建新表
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        Self::ensure_schema(&conn)?;

        Ok(Self {conn: Mutex::new(conn)})
    }

    /// 确保 `instance_states` 表为 v2 结构, 必要时重建.
    fn ensure_schema(conn: &Connection) -> Result<()> {
        let has_table: bool = conn.query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'instance_states'",
            [],
            |_| Ok(true),
        ).unwrap_or(false);

        if has_table && Self::has_legacy_banner_id_column(conn)? {
            tracing::warn!(
                "检测到 v1 数据格式的 instance_states 表 (banner_id 整数主键), \
                按 v2 数据格式重建, 旧的卡池状态将丢失"
            );
            conn.execute("DROP TABLE instance_states", [])?;
        }

        conn.execute(
            r#"
                CREATE TABLE IF NOT EXISTS instance_states (
                    scope_key       TEXT NOT NULL,
                    user_id         INTEGER NOT NULL DEFAULT 0,
                    total_counter   INTEGER NOT NULL DEFAULT 0,
                    logic_state     TEXT,
                    PRIMARY KEY     (scope_key)
                )
            "#,
            []
        )?;

        Ok(())
    }

    /// 表结构是否为 v1 的 `banner_id` 形式.
    fn has_legacy_banner_id_column(conn: &Connection) -> Result<bool> {
        let mut stmt = conn.prepare("PRAGMA table_info(instance_states)")?;
        let mut rows = stmt.query([])?;

        while let Some(row) = rows.next()? {
            let name: String = row.get(1)?;
            if name == "banner_id" {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// 加载指定用户和状态作用域的状态, 若不存在则返回 `None`.
    /// 
    /// 当前为单用户模式.
    pub fn load(&self,
        user_id: UserId,
        scope_key: &str
    ) -> Result<Option<BannerRuntimeState>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT user_id, total_counter, logic_state FROM instance_states WHERE scope_key = ?1"
        )?;
        let mut rows = stmt.query(rusqlite::params![scope_key])?;

        if let Some(row) = rows.next()? {
            let stored_user_i64: i64 = row.get(0)?;
            let stored_user = u64::try_from(stored_user_i64)
                .with_context(|| format!(
                    "scope_key `{}` 的 user_id ({}) 无法转换为 u64",
                    scope_key, stored_user_i64
                ))?;
            
            if stored_user != user_id.0 {
                // 当前为单用户模式; 多用户隔离实现时, 这里需改为按 (user_id, scope_key) 查询
                tracing::warn!(
                    scope_key = %scope_key,
                    stored_user = %stored_user,
                    requested_user = %user_id.0,
                    "状态所属用户与请求用户不一致, 视为不存在"
                );
                return Ok(None);
            }

            let total_counter_i64: i64 = row.get(1)?;
            let total_counter = u64::try_from(total_counter_i64)
                .with_context(|| format!(
                    "scope_key `{}` 的 total_counter (value {}) 无法转换为 u64",
                    scope_key, total_counter_i64
                ))?;
            
            let logic_state_str: Option<String> = row.get(2)?;
            let logic_state = match logic_state_str {
                Some(s) => serde_json::from_str(&s)
                    .with_context(|| format!(
                        "scope_key `{}` 的 logic_state 反序列化失败",
                        scope_key
                    ))?,
                None => JsonValue::Null,
            };

            Ok(Some(BannerRuntimeState{
                total_counter,
                logic_state,
            }))
        } else {
            Ok(None)
        }
    }

    /// 保存或更新指定用户和状态作用域的状态.
    pub fn save(&self,
        user_id: UserId,
        scope_key: &str,
        state: &BannerRuntimeState
    ) -> Result<()> {
        let total_counter_i64 = i64::try_from(state.total_counter)
            .with_context(|| format!(
                "scope_key `{}` 的 total_counter (value {}) 无法转换为 i64",
                scope_key, state.total_counter
            ))?;
        
        let logic_state_str = if state.logic_state.is_null() {
            None
        } else {
            Some(
                serde_json::to_string(&state.logic_state)
                    .with_context(|| format!(
                        "scope_key `{}` 的 logic_state 序列化为 JSON 文本失败",
                        scope_key
                    ))?
            )
        };
        
        let conn = self.conn.lock();
        conn.execute(
            r#"
                INSERT INTO instance_states (scope_key, user_id, total_counter, logic_state)
                VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT(scope_key) DO UPDATE SET
                    user_id = excluded.user_id,
                    total_counter = excluded.total_counter,
                    logic_state = excluded.logic_state;
            "#,
            rusqlite::params![scope_key, user_id, total_counter_i64, logic_state_str]
        )?;
        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use uuid::Uuid;
    use super::*;

    fn temp_db_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("wishes-state-{}-{}.db", tag, Uuid::now_v7()))
    }

    #[test]
    fn saves_and_loads_state_by_scope_key() {
        let path = temp_db_path("roundtrip");
        let repo = StateRepository::new(&path).unwrap();

        let state = BannerRuntimeState {
            total_counter: 42,
            logic_state: serde_json::json!({ "counter_5": 3 }),
        };

        repo.save(UserId(0), "banner:abc", &state).unwrap();
        let loaded = repo.load(UserId(0), "banner:abc").unwrap().unwrap();

        assert_eq!(loaded.total_counter, 42);
        assert_eq!(loaded.logic_state["counter_5"], 3);
        assert!(repo.load(UserId(0), "banner:other").unwrap().is_none());

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn group_scope_shares_one_row() {
        let path = temp_db_path("group");
        let repo = StateRepository::new(&path).unwrap();
        let key = "group:arknights-standard";

        repo.save(UserId(0), key, &BannerRuntimeState { total_counter: 10, logic_state: JsonValue::Null }).unwrap();
        repo.save(UserId(0), key, &BannerRuntimeState { total_counter: 11, logic_state: JsonValue::Null }).unwrap();

        let loaded = repo.load(UserId(0), key).unwrap().unwrap();
        assert_eq!(loaded.total_counter, 11, "同一作用域只应保留一份状态");

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn legacy_schema_is_rebuilt() {
        let path = temp_db_path("legacy");
        {
            let conn = Connection::open(&path).unwrap();
            // 旧表结构
            conn.execute(
                "CREATE TABLE instance_states (
                    user_id INTEGER NOT NULL,
                    banner_id INTEGER NOT NULL,
                    total_counter INTEGER NOT NULL DEFAULT 0,
                    logic_state Text,
                    PRIMARY KEY (user_id, banner_id)
                )",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO instance_states (user_id, banner_id, total_counter) VALUES (0, 1, 77)",
                [],
            ).unwrap();
        }

        let repo = StateRepository::new(&path).unwrap();

        // 旧表被重建, 旧数据不再存在, 但新结构可以正常写入
        assert!(repo.load(UserId(0), "banner:1").unwrap().is_none());
        repo.save(UserId(0), "banner:1", &BannerRuntimeState::default()).unwrap();
        assert!(repo.load(UserId(0), "banner:1").unwrap().is_some());

        std::fs::remove_file(&path).ok();
    }
}