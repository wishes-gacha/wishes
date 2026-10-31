//! # 逻辑定义 (静态蓝图) 相关类型 (数据格式 v2)

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use crate::{
    domain::{
        ids::{GlobalId, WishRuleLocalId},
        localized_string::LocalizedString,
        origin::Origin, tag::Tagged
    }
};


/// 规则类型枚举.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum RuleType {
    /// 内置规则, 使用 Rust 代码实现, 拥有更好的性能.
    Builtin {
        /// 内置规则名称.
        builtin_name: String,
    },
    /// 自定义规则, 由蓝图等其他方式实现 (未来拓展)
    Custom {
        script_path: String,
        properties: JsonValue,
    }
}


/// 静态规则定义蓝本 (单个规则).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleDefinition {
    /// 规则在逻辑内部唯一的唯一标识, 用于状态字典的键.
    pub local_id: WishRuleLocalId,
    /// 规则类型.
    pub rule_type: RuleType,
    /// 规则的静态参数.
    pub params: JsonValue,
}


/// 逻辑变体枚举.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "variant", content = "config")]
pub enum LogicVariant {
    /// 硬编码逻辑, 由 Rust 代码直接实现, 无规则链调用, 性能更好.
    Hardcoded {
        /// 内置执行器名称.
        /// 目前已实现的执行器包括:
        /// - `genshin_character_up`: 原神角色 UP 卡池逻辑 (含捕获明光)
        /// - `starrail_character_up`: 崩坏星穹铁道角色 UP 卡池逻辑
        executor_name: String,
    },
    /// 基于规则链的组合式逻辑.
    RuleBased {
        /// 有序规则列表, 按顺序执行内部的规则.
        rules: Vec<RuleDefinition>,
    }
}

/// 全局的静态逻辑定义蓝本.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogicDefinition {
    /// 全局唯一标识.
    pub global_id: GlobalId,

    /// 对象来源.
    pub origin: Origin,

    /// 派生自哪个对象的 `global_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forked_from: Option<GlobalId>,

    /// 派生对象是否遮蔽其原对象.
    /// 
    /// - `true` (默认): 原对象在列表与抽卡查询中被跳过.
    /// - `false`: 原对象仍可见, 用户可以同时看到原对象与派生对象.
    /// 
    /// 对 `forked_from = None` 的对象该字段无意义.
    #[serde(default = "crate::domain::helpers::default_true", skip_serializing_if = "crate::domain::helpers::is_true")]
    pub shadowed: bool,

    /// 逻辑名称.
    pub name: LocalizedString,

    /// 逻辑变体.
    pub variant: LogicVariant,
}

impl LogicDefinition {
    /// 以默认来源 (`local`) 创建一个逻辑定义.
    pub fn new(name: LocalizedString, variant: LogicVariant) -> Self {
        Self {
            global_id: GlobalId::new(),
            origin: Origin::Local,
            forked_from: None,
            shadowed: true,
            name,
            variant,
        }
    }

    /// 按给定语言解析逻辑显示名, 缺失时回退到默认语言或任意可用语言.
    pub fn display_name(&self, locale: &str) -> Option<&str> {
        self.name.get_or_default_locale(locale)
    }
}

/// 带标签的逻辑定义.
/// 即 `Tagged<LogicDefinition>`
pub type TaggedLogicDefinition = Tagged<LogicDefinition>;


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_according_to_v2_layout() {
        let def = LogicDefinition::new(
            LocalizedString::single("zh-CN", "原神角色 UP 池逻辑"),
            LogicVariant::Hardcoded { executor_name: "genshin_character_up".into() },
        );

        let value: serde_json::Value = serde_json::to_value(&def).unwrap();

        assert!(value["global_id"].is_string());
        assert_eq!(value["origin"], "local");
        assert_eq!(value["name"]["zh-CN"], "原神角色 UP 池逻辑");
        assert_eq!(value["variant"]["variant"], "Hardcoded");
        assert_eq!(value["variant"]["config"]["executor_name"], "genshin_character_up");
        assert!(value.get("forked_from").is_none());
    }
}
