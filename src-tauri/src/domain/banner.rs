//! # 卡池 Banner 定义
//! 关联卡组和逻辑, 并持有运行时状态状态实例.

use serde::{Serialize, Deserialize};
use crate::domain::{ids::GlobalId, localized_string::LocalizedString, origin::Origin, tag::Tagged};
use serde_json::Value as JsonValue;

/// `state_scope` 的默认值: 每个卡池独立维护保底状态.
pub const STATE_SCOPE_DEFAULT: &str = "banner";

/// 共享状态的作用域前缀, 完整形式为 `"group:<name>"`.
pub const STATE_SCOPE_GROUP_PREFIX: &str = "group:";

fn default_state_scope() -> String {
    STATE_SCOPE_DEFAULT.to_string()
}

/// 卡池核心数据.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Banner {
    /// 全局唯一标识.
    pub global_id: GlobalId,

    /// 对象来源. 即使为 `"local"` 也始终序列化.
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

    /// 卡池名称 (至少一种语言).
    pub name: LocalizedString,

    /// 资源引用.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assets: Option<BannerAssets>,

    /// 关联的卡组 `global_id`.
    pub deck_id: GlobalId,

    /// 逻辑 `global_id`.
    pub logic_id: GlobalId,

    /// 运行时状态的作用域.
    /// 
    /// - `"banner"` (默认): 每个卡池独立维护保底状态.
    /// - `"group:<name>"`: 同组的卡池共享状态 (如方舟标准池跨池继承).
    /// 
    /// **始终序列化**, 即使是默认值 —— 它是影响抽卡行为的重要字段,
    /// 显式写出可避免人工阅读时误判.
    #[serde(default = "default_state_scope")]
    pub state_scope: String,
}

impl Banner {
    /// 以默认来源 (`local`)、默认作用域创建一个卡池.
    pub fn new(name: LocalizedString, deck_id: GlobalId, logic_id: GlobalId) -> Self {
        Self {
            global_id: GlobalId::new(),
            origin: Origin::Local,
            forked_from: None,
            shadowed: true,
            name,
            assets: None,
            deck_id,
            logic_id,
            state_scope: default_state_scope(),
        }
    }

    /// 按给定语言解析卡池显示名, 缺失时回退到默认语言或任意可用语言.
    pub fn display_name(&self, locale: &str) -> Option<&str> {
        self.name.get_or_default_locale(locale)
    }

    /// 该卡池是否使用共享状态作用域 (`group:<name>`).
    pub fn shares_state(&self) -> bool {
        self.state_scope.starts_with(STATE_SCOPE_GROUP_PREFIX)
    }

    /// 运行时状态的存储键 (`scope_key`).
    /// 
    /// 形如 `"banner:<uuid>"` 或 `"group:<name>"`.
    /// 
    /// 所有使用相同 `scope_key` 的卡池共享同一份运行时状态.
    /// 这是 v2 相对 v1 (`instance_states.banner_id` 整数) 的关键变化:
    /// 状态不再以卡池本身为唯一键, 而是以"状态作用域"为键.
    pub fn state_scope_key(&self) -> String {
        if self.shares_state() {
            self.state_scope.clone()
        } else {
            format!("banner:{}", self.global_id)
        }
    }
}

/// 带标签的卡池.
/// 即 `Tagged<Banner>`.
pub type TaggedBanner = Tagged<Banner>;

/// 卡池运行时状态
/// 
/// 与 `Banner` 分离存储: `Banner` 是不可变的配置, `BannerRuntimeState` 是可变的进度.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BannerRuntimeState {
    /// 该作用域下的总抽数.
    #[serde(default)]
    pub total_counter: u64,

    /// 逻辑自身的状态 (由逻辑自由定义, 通常是 JSON 映射表).
    #[serde(default)]
    pub logic_state: JsonValue,
}

impl Default for BannerRuntimeState {
    fn default() -> Self {
        Self {
            total_counter: 0,
            logic_state: JsonValue::Null,
        }
    }
}

impl BannerRuntimeState {
    /// 分叉卡池时使用的初始状态: 全部归零.
    /// 
    /// 分叉的语义是"基于原配置创建一个新对象", 新对象应从零开始;
    /// "想保留保底进度改名"是**重命名**而非分叉, 两者语义应明确区分.
    pub fn initial() -> Self {
        Self::default()
    }
}

/// 卡池资产引用.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BannerAssets {
    /// 抽卡页大背景 (预留, 具体用途待后续讨论).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,

    /// 主页展示的卡池封面图标.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,

    /// 抽卡页轮播展示的卡片列表 (预留).
    /// 
    /// 引用 Card 的 `global_id`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub carousel_cards: Vec<GlobalId>,
}

impl BannerAssets {
    /// 是否所有资产引用都为空.
    pub fn is_empty(&self) -> bool {
        self.background.is_none() && self.icon.is_none() && self.carousel_cards.is_empty()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn banner_with_scope(scope: &str) -> Banner {
        let mut banner = Banner::new(
            LocalizedString::single("zh-CN", "赤团开时"),
            GlobalId::new(),
            GlobalId::new(),
        );
        banner.state_scope = scope.to_string();
        banner
    }

    #[test]
    fn state_scope_is_always_serialized_even_when_default() {
        let banner = banner_with_scope(STATE_SCOPE_DEFAULT);
        let value: serde_json::Value = serde_json::to_value(&banner).unwrap();

        assert_eq!(value["state_scope"], "banner");
    }

    #[test]
    fn state_scope_key_is_banner_uuid_by_default() {
        let banner = banner_with_scope(STATE_SCOPE_DEFAULT);

        assert_eq!(banner.state_scope_key(), format!("banner:{}", banner.global_id));
        assert!(!banner.shares_state());
    }

    #[test]
    fn state_scope_key_is_shared_for_group_scope() {
        let a = banner_with_scope("group:arknights-standard");
        let b = banner_with_scope("group:arknights-standard");

        assert!(a.shares_state());
        assert_eq!(a.state_scope_key(), "group:arknights-standard");
        assert_eq!(a.state_scope_key(), b.state_scope_key(), "同组卡池必须共享状态键");
        assert_ne!(a.global_id, b.global_id);
    }

    #[test]
    fn default_state_scope_applies_when_field_missing() {
        let json = format!(
            r#"{{
                "global_id": "{}",
                "origin": "official",
                "name": {{"zh-CN": "赤团开时"}},
                "deck_id": "{}",
                "logic_id": "{}"
            }}"#,
            GlobalId::new(),
            GlobalId::new(),
            GlobalId::new(),
        );

        let banner: Banner = serde_json::from_str(&json).unwrap();
        assert_eq!(banner.state_scope, STATE_SCOPE_DEFAULT);
        assert_eq!(banner.global_id.short().len(), 8);
    }
}
