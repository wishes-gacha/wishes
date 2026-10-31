//! # 对象来源 `Origin`
//! 
//! 数据格式 v2 起, 每个核心对象都记录自己来自哪里, 以支撑**数据分叉模型**:
//! 
//! - `official`: 官方数据, 位于 `data/official/`, **永远只读**.
//! - `local`: 用户数据, 位于 `data/local/`, 可自由创建与修改.
//! - `pack:<name>`: 拓展包数据, 位于 `data/packs/<name>/`, **永远只读**.
//! 
//! 用户对官方/拓展包对象的任何"编辑"都是**新建一个派生对象** (`origin = local`,
//! 并记录 `forked_from`), 原对象从不被改写. 由此冲突不可能发生.

use std::fmt;
use serde::{Deserialize, Serialize};

/// 对象来源.
/// 
/// 序列化为单个字符串, 与数据文件中的 `origin` 字段一一对应:
/// `"official"` / `"local"` / `"pack:<name>"`.
/// 
/// 即使是默认值 `"local"` 也**始终序列化** —— 它是重要的语义标识,
/// 显式写出便于调试与人工阅读.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Origin {
    /// 官方数据 (`"official"`).
    Official,
    /// 用户数据 (`"local"`).
    Local,
    /// 拓展包数据 (`"pack:<name>"`).
    Pack(String),
}

impl Origin {
    /// 来源的字符串表示, 与序列化格式一致.
    /// 
    /// - `"official"` / `"local"`
    /// - `"pack:<name>"`
    pub fn as_str(&self) -> String {
        match self {
            Origin::Official => "official".to_string(),
            Origin::Local => "local".to_string(),
            Origin::Pack(name) => format!("pack:{}", name),
        }
    }

    /// 由字符串解析来源.
    /// 
    /// `"pack:"` 前缀后为空视为非法.
    pub fn parse(s: &str) -> Result<Self, OriginParseError> {
        match s {
            "official" => Ok(Origin::Official),
            "local" => Ok(Origin::Local),
            other => match other.strip_prefix("pack:") {
                Some(name) if !name.is_empty() => Ok(Origin::Pack(name.to_string())),
                _ => Err(OriginParseError(other.to_string())),
            },
        }
    }

    /// 该来源的数据是否**只读** (官方与拓展包).
    /// 
    /// 只读来源的对象不可被直接修改, 只能派生.
    pub fn is_read_only(&self) -> bool {
        !matches!(self, Origin::Local)
    }

    /// 是否为用户本地数据.
    pub fn is_local(&self) -> bool {
        matches!(self, Origin::Local)
    }

    /// 是否为官方数据.
    pub fn is_official(&self) -> bool {
        matches!(self, Origin::Official)
    }
}

impl Default for Origin {
    /// 默认来源为 `local`.
    /// 
    /// 新建对象时若未显式指定来源, 一律视为用户数据.
    fn default() -> Self {
        Origin::Local
    }
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 来源字符串解析失败.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("非法的 Origin 字符串: `{0}` (应为 \"official\"、\"local\" 或 \"pack:<name>\")")]
pub struct OriginParseError(pub String);

impl Serialize for Origin {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.as_str())
    }
}

impl<'de> Deserialize<'de> for Origin {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Origin::parse(&raw).map_err(serde::de::Error::custom)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_as_plain_string() {
        assert_eq!(serde_json::to_string(&Origin::Official).unwrap(), r#""official""#);
        assert_eq!(serde_json::to_string(&Origin::Local).unwrap(), r#""local""#);
        assert_eq!(
            serde_json::to_string(&Origin::Pack("genshin-pack".into())).unwrap(),
            r#""pack:genshin-pack""#
        );
    }

    #[test]
    fn parses_back_from_string() {
        for origin in [Origin::Official, Origin::Local, Origin::Pack("abc".into())] {
            let json = serde_json::to_string(&origin).unwrap();
            let parsed: Origin = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, origin);
        }
    }

    #[test]
    fn rejects_invalid_origin_strings() {
        assert!(Origin::parse("pack:").is_err());
        assert!(Origin::parse("社区包").is_err());
        assert!(Origin::parse("").is_err());
    }

    #[test]
    fn read_only_sources_are_not_local() {
        assert!(Origin::Official.is_read_only());
        assert!(Origin::Pack("p".into()).is_read_only());
        assert!(!Origin::Local.is_read_only());
    }
}
