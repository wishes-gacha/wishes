//! # 卡组接口响应与请求

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{domain::{deck::{DeckAssets, EventGroup, Membership, TaggedDeck}, ids::GlobalId, localized_string::LocalizedString, origin::Origin, tag::{EventTag, Tag}}, infrastructure::registry::DeckRegistry};


/// 卡组摘要, 用于列表和编辑.
#[derive(Debug, Serialize)]
pub struct DeckSummary {
    /// 卡组 `global_id`.
    pub global_id: GlobalId,

    /// 对象来源 (`official` / `local` / `pack:<name>`).
    pub origin: Origin,

    /// 派生自哪个对象的 `global_id`.
    pub forked_from: Option<GlobalId>,

    /// 所派生的原对象的名称, 用于显示 (多语言).
    pub forked_from_name: Option<LocalizedString>,

    /// 卡组名称 (多语言).
    pub name: LocalizedString,

    /// 资产引用.
    pub assets: Option<DeckAssets>,

    /// 成员规则.
    pub members: Membership,

    /// 活动标签分组.
    pub event_groups: HashMap<EventTag, EventGroup>,

    /// 标签 (按命名空间排序).
    pub tags: Vec<Tag>,
    
}

impl From<&TaggedDeck> for DeckSummary {
    fn from(deck: &TaggedDeck) -> Self {
        Self {
            global_id: deck.global_id,
            origin: deck.origin.clone(),
            forked_from: deck.forked_from,
            forked_from_name: None,
            name: deck.name.clone(),
            assets: deck.assets.clone(),
            members: deck.members.clone(),
            event_groups: deck.event_groups.clone(),
            tags: deck.sorted_tags(),
        }
    }
}


impl DeckSummary {
    pub fn from_with_registry(deck: &TaggedDeck, registry: &DeckRegistry) -> Self {
        let forked_from_name = deck.forked_from
            .and_then(|id| registry.get_including_shadowed(id))
            .map(|d| d.name.clone());
        Self {
            forked_from_name,
            ..Self::from(deck)
        }
    }
}


/// 创建卡组的请求.
#[derive(Debug, Deserialize)]
pub struct CreateDeckRequest {
    /// 卡组名称 (多语言).
    pub name: LocalizedString,

    /// 成员规则.
    pub members: Membership,
    
    /// 活动标签分组.
    #[serde(default)]
    pub event_groups: HashMap<EventTag, EventGroup>,

    /// 标签.
    #[serde(default)]
    pub tags: Vec<Tag>,
}


/// 更新卡组的请求.
/// 
/// 若目标卡组来自只读来源 (官方 / 拓展包), 该操作会**分叉**出一个本地副本.
/// 注意: 引用该卡组的卡池**不会**被自动切换, 需要 UI 提示用户.
#[derive(Debug, Deserialize)]
pub struct UpdateDeckRequest {
    /// 目标卡组的 `global_id`.
    pub global_id: GlobalId,

    /// 新名称 (多语言).
    #[serde(default)]
    pub name: Option<LocalizedString>,

    /// 新成员规则.
    #[serde(default)]
    pub members: Option<Membership>,
    
    /// 新活动标签分组.
    #[serde(default)]
    pub event_groups: Option<HashMap<EventTag, EventGroup>>,
    
    /// 新标签.
    #[serde(default)]
    pub tags: Option<Vec<Tag>>,
}