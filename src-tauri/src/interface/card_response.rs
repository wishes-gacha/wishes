//! # 卡片接口响应与请求
//! 
//! 数据格式 v2 起, 对象标识统一为 `global_id` (Uuid 字符串),
//! 文本字段使用 `LocalizedString` (语言标签 -> 文本 的映射),
//! 由前端按当前界面语言回退解析.

use serde::{Deserialize, Serialize};
use crate::{domain::{card::{CardAssets, TaggedCard}, ids::GlobalId, localized_string::LocalizedString, origin::Origin, tag::Tag}, infrastructure::registry::CardRegistry};


/// 卡片摘要, 用于列表与详情展示.
#[derive(Debug, Clone, Serialize)]
pub struct CardSummary {
    /// 卡片 `global_id`.
    pub global_id: GlobalId,

    /// 对象来源 (`official` / `local` / `pack:<name>`).
    pub origin: Origin,

    /// 派生自哪个对象的 `global_id`.
    pub forked_from: Option<GlobalId>,

    /// 所派生的原对象的名称, 用于显示 (多语言).
    pub forked_from_name: Option<LocalizedString>,

    /// 在对象为派生对象时, 对象是否对原对象遮蔽.
    pub shadowed: bool,

    /// 若具有派生对象 (自身为原对象), 保存派生对象的 `global_id`.
    /// 仅用于让前端知道对象是否被派生.
    pub derived_id: Option<GlobalId>,

    /// 卡片内容 (多语言).
    pub content: LocalizedString,

    /// 称号 / 副标题 (多语言).
    pub title: Option<LocalizedString>,
    
    /// 资产引用.
    pub assets: Option<CardAssets>,

    /// 标签 (按命名空间排序).
    pub tags: Vec<Tag>,
}

impl From<&TaggedCard> for CardSummary {
    fn from(card: &TaggedCard) -> Self {
        Self {
            global_id: card.global_id,
            origin: card.origin.clone(),
            forked_from: card.forked_from,
            forked_from_name: None,
            derived_id: None,
            shadowed: card.shadowed,
            content: card.content.clone(),
            title: card.title.clone(),
            assets: card.assets.clone(),
            tags: card.sorted_tags(),
        }
    }
}


impl CardSummary {
    pub fn from_with_registry(card: &TaggedCard, registry: &CardRegistry) -> Self {
        let forked_from_name = card.forked_from
            .and_then(|id| registry.get_including_shadowed(id))
            .map(|c| c.content.clone());

        let mut derived = registry.derived_of(card.global_id);
        derived.sort();
        let derived_id = derived.first().copied();

        Self {
            forked_from_name,
            derived_id,
            ..Self::from(card)
        }
    }
}


/// 创建卡片的请求.
/// 
/// 文本字段为完整的 `LocalizedString`; 可以只填写当前语言,
/// 后端会校验"至少一种语言非空".
#[derive(Debug, Clone, Deserialize)]
pub struct CardCreateRequest {
    /// 卡片内容 (多语言).
    pub content: LocalizedString,

    /// 称号 / 副标题 (多语言).
    #[serde(default)]
    pub title: Option<LocalizedString>,

    /// 卡片标签.
    #[serde(default)]
    pub tags: Vec<Tag>,
}


/// 更新卡片的请求.
/// 
/// 若目标卡片来自只读来源 (官方 / 拓展包), 该操作会**分叉**出一个本地副本,
/// 原对象保持不变.
#[derive(Debug, Clone, Deserialize)]
pub struct CardUpdateRequest {
    /// 目标卡片的 `global_id`.
    pub global_id: GlobalId,

    /// 新的卡片内容 (多语言).
    pub content: LocalizedString,

    /// 新的称号 / 副标题 (多语言).
    #[serde(default)]
    pub title: Option<LocalizedString>,

    /// 新的卡片标签.
    #[serde(default)]
    pub tags: Vec<Tag>,

    /// 是否遮蔽.
    #[serde(default)]
    pub shadow: Option<bool>,
}