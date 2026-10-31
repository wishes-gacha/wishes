//! # 领域对象的标识类型
//! 
//! 数据格式 v2 起, 所有核心对象 (Card / Deck / LogicDefinition / Banner)
//! 统一使用 `GlobalId` 作为**唯一**标识, 不再保留本地自增 `u64` Id.
//! 
//! 这样所有跨对象引用 (`Deck` 的 `include_ids`、`Banner` 的 `deck_id` 等)
//! 都是同一种类型, 不存在"该用哪个 Id"的决策点, 也不再需要 `IdAllocator`
//! 以及 `Uuid <-> u64` 映射表.
//! 
//! # 为什么是 Uuid v7
//! 
//! - 官方数据同步、拓展包导入、派生对象等场景都需要**跨实例唯一**标识.
//! - v7 为时间有序 (前 48 位是毫秒时间戳), 作为索引键的局部性远优于 v4,
//!   在 `Wishes` 的规模 (后期约千张卡片) 下性能损失可忽略.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// 全局唯一标识 (`Uuid` v7).
/// 
/// 序列化为标准 Uuid 字符串 (如 `"0192a3c4-1234-7abc-8def-000000000001"`),
/// 因此前端与数据文件中看到的都是字符串.
/// 
/// # 示例
/// 
/// ```
/// # use wishes_lib::domain::ids::GlobalId;
/// let id = GlobalId::new();
/// assert_eq!(id.0.get_version_num(), 7);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GlobalId(pub Uuid);

impl GlobalId {
    /// 生成一个新的 `GlobalId` (Uuid v7, 时间有序).
    /// 
    /// 创建对象时调用一次, 之后**永不变更**.
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }

    /// 由已有的 `Uuid` 构造.
    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// 由 Uuid 字符串解析.
    pub fn parse(s: &str) -> Result<Self, uuid::Error> {
        Uuid::parse_str(s).map(Self)
    }

    /// 内部 `Uuid` 的引用.
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// 标准 Uuid 字符串.
    pub fn to_hyphenated(&self) -> String {
        self.0.hyphenated().to_string()
    }

    /// 用于界面展示的短标识 (前 8 位十六进制, 如 `0192a3c4`).
    /// 
    /// 完整 Uuid 过长, 不适合在列表中展示.
    pub fn short(&self) -> String {
        self.to_hyphenated().chars().take(8).collect()
    }
}

impl Default for GlobalId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for GlobalId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hyphenated())
    }
}

impl From<Uuid> for GlobalId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl From<GlobalId> for Uuid {
    fn from(value: GlobalId) -> Self {
        value.0
    }
}

/// 用户 Id (用于多用户隔离, 目前阶段固定为 0).
/// 
/// 多用户隔离是**本地实例内**的概念, 不参与跨实例数据交换,
/// 因此它不是 `GlobalId`, 仍是简单的 `u64`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct UserId(pub u64);

impl From<u64> for UserId {
    fn from(id: u64) -> Self {
        UserId(id)
    }
}

/// 抽卡规则 Id (逻辑内部的唯一标识, String 形式).
/// 
/// 作用域限于单个 `LogicDefinition` 内部, 用作状态字典的键, 因此不需要是 `GlobalId`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WishRuleLocalId(pub String);

impl From<String> for WishRuleLocalId {
    fn from(value: String) -> Self {
        WishRuleLocalId(value)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_id_is_uuid_v7_and_time_ordered() {
        let a = GlobalId::new();
        let b = GlobalId::new();

        assert_eq!(a.0.get_version_num(), 7);
        assert!(a < b, "Uuid v7 应按生成时间递增: {a} < {b}");
    }

    #[test]
    fn serializes_as_uuid_string() {
        let id = GlobalId::parse("0192a3c4-1234-7abc-8def-000000000001").unwrap();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, r#""0192a3c4-1234-7abc-8def-000000000001""#);

        let parsed: GlobalId = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn short_is_first_8_hex_chars() {
        let id = GlobalId::parse("0192a3c4-1234-7abc-8def-000000000001").unwrap();
        assert_eq!(id.short(), "0192a3c4");
    }
}
