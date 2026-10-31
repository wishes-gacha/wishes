//! # 逻辑模块常见错误类型定义

use crate::domain::ids::GlobalId;



/// 抽卡流程中发生的逻辑模块错误.
#[derive(Debug, thiserror::Error)]
pub enum LogicError {
    /// 逻辑定义未找到
    #[error("Logic definition not found: {0:?}")]
    DefinitionNotFound(GlobalId),
    /// `Variant::Hardcoded` 类型逻辑定义所关联的硬编码执行器未找到.
    #[error("Logic executor not found: {0}")]
    ExecutorNotFound(String),
    /// `Variant::RuleBased` 类型逻辑定义所包含的规则执行器未找到.
    #[error("Rule executor not found: {0}")]
    RuleNotFound(String),
    /// `Variant::RuleBased` 类型逻辑定义所包含的自定义规则未找到.
    #[error("Custom rule not found: {0}")]
    CustomRuleNotFound(String),
    /// 自定义规则当前不可用.
    #[error("Custom rule is not usable now")]
    CustomRuleUnusable(),
}