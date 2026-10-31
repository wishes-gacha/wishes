//! # 卡池信息
//! 
//! 提供卡池的摘要信息 (`BannerSummary`) 和 详细信息 (`BannerInfo`), 
//! 分别用于列表展示和详情页展示.

use serde::Serialize;
use crate::domain::{banner::TaggedBanner, ids::GlobalId, localized_string::LocalizedString, origin::Origin, tag::Tag};

/// 卡池的摘要信息, 用于列表或概览展示.
#[derive(Debug, Serialize)]
pub struct BannerSummary {
    /// 卡池 `global_id`.
    pub global_id: GlobalId,

    /// 对象来源 ('official' / 'local' 'pack:<name>').
    pub origin: Origin,

    /// 派生自哪个对象.
    pub forked_from: Option<GlobalId>,

    /// 卡池名称 (多语言).
    pub name: LocalizedString,

    /// 卡池拥有的标签 (按命名空间排序).
    pub tags: Vec<Tag>,
}

impl From<&TaggedBanner> for BannerSummary {
    fn from(banner: &TaggedBanner) -> Self {
        Self {
            global_id: banner.global_id,
            origin: banner.origin.clone(),
            forked_from: banner.forked_from,
            name: banner.name.clone(),
            tags: banner.sorted_tags(),
        }
    }
}

/// 卡池的详细信息, 用于详情页或管理界面.
#[derive(Debug, Serialize)]
pub struct BannerInfo {
    /// 卡池 `global_id`.
    pub global_id: GlobalId,

    /// 卡池名称 (多语言).
    pub name: LocalizedString,

    /// 卡池拥有的标签.
    pub tags: Vec<Tag>,

    /// 卡池所引用的卡组 `global_id`.
    pub deck_id: GlobalId,

    /// 卡池所引用的逻辑定义 `global_id`.
    pub logic_id: GlobalId,

    /// 卡池所引用的卡组名称 (多语言).
    pub deck_name: LocalizedString,

    /// 卡池所引用的逻辑定义名称 (多语言).
    pub logic_name: LocalizedString,

    /// 运行时状态作用域 (`"banner"` 或 `"group:<name>"`).
    pub state_scope: String,

    /// 该卡池的**状态作用域**总抽卡次数.
    pub total_counter: u64,
}