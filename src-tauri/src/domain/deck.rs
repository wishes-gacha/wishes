//! # 卡组 Deck 定义
//! 包含卡片 `Card` 成员和活动标签分组.

use std::collections::{HashMap, HashSet};
use serde::{Serialize, Deserialize};

use crate::{
    domain::{
        ids::GlobalId, localized_string::LocalizedString, origin::Origin, tag::{EventTag, Tag, Tagged}
    }, infrastructure::registry::CardRegistry
};


/// 某个卡组对指定卡片的显式引用情况.
///
/// 仅统计 `include_ids` / `exclude_ids` 这类显式声明, 不含标签规则的动态包含.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CardRefs {
    /// 通过 `include_ids` 显式包含.
    pub included: bool,
    /// 通过 `exclude_ids` 显式排除.
    pub excluded: bool,
}

impl CardRefs {
    /// 是否存在任一方向的显式引用.
    pub fn is_any(&self) -> bool {
        self.included || self.excluded
    }
}


/// 声明卡组成员卡片的条件.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MembershipCondition {
    /// 全局拉取: 拥有所有指定标签的卡片.
    #[serde(rename = "tag_all")]
    TagAll{ tags: Vec<Tag> },

    /// 全局拉取: 拥有任意指定标签的卡片.
    #[serde(rename = "tag_any")]
    TagAny{ tags: Vec<Tag> },

    /// 本地过滤: 保留当前结果中拥有所有指定标签的卡片.
    #[serde(rename = "filter_tag_all")]
    FilterTagAll{ tags: Vec<Tag> },

    /// 本地过滤: 保留当前结果中拥有任意指定标签的卡片.
    #[serde(rename = "filter_tag_any")]
    FilterTagAny{tags: Vec<Tag>},

    /// 全局拉取: 直接添加这些 `global_id` 的卡片.
    #[serde(rename = "include_ids")]
    IncludeIds{ ids: HashSet<GlobalId> },

    /// 排除: 直接排除这些指定的卡片.
    #[serde(rename = "exclude_ids")]
    ExcludeIds{ ids: HashSet<GlobalId> },

    // TODO[2026-08-19]: 当前的匹配规则主要是正向匹配, 未来加入标签反向排除规则, EventGroupCondition 同理
}

impl MembershipCondition {
    /// 该条件显式引用 (包含) 的卡片 `global_id` 集合.
    /// 
    /// 用于分叉时的引用重定向与删除前的引用检查
    pub fn explicit_card_refs(&self) -> Option<&HashSet<GlobalId>> {
        match self {
            MembershipCondition::IncludeIds { ids } => Some(ids),
            MembershipCondition::ExcludeIds { ids } => Some(ids),
            _ => None,
        }
    }

    /// 把 `include_ids` / `exclude_ids` 中指向 `from` 的引用替换为 `to`.
    /// 
    /// 返回是否发生了替换 (用于统计分叉时重定向了多少处引用).
    pub fn redirect_card_refs(&mut self, from: GlobalId, to: GlobalId) -> bool {
        let ids = match self {
            MembershipCondition::IncludeIds { ids } => ids,
            MembershipCondition::ExcludeIds { ids } => ids,
            _ => return false,
        };

        if ids.remove(&from) {
            ids.insert(to);
            true
        } else {
            false
        }
    }
}

/// 卡组成员规则, 每个条件顺序应用.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Membership {
    /// 按顺序应用的条件列表.
    #[serde(default)]
    pub conditions: Vec<MembershipCondition>,
}

impl Membership {
    /// 由成员规则计算得到最终成员卡片
    /// 
    /// # 执行顺序
    /// 1. 顺序应用全部包含型与过滤型条件 (顺序会影响结果).
    /// 2. 应用全部排除型条件 (顺序不影响结果).
    pub fn resolve(&self, registry: &CardRegistry) -> HashSet<GlobalId> {
        let mut result = HashSet::new();

        // 1. 应用所有包含型和过滤型条件, 顺序能影响执行效果
        for cond in &self.conditions {
            match cond {
                MembershipCondition::TagAll { tags } => {
                    if !tags.is_empty() {
                        let ids = registry.tag_index.query_all(tags);
                        result.extend(ids);
                    }
                },
                MembershipCondition::TagAny { tags } => {
                    if !tags.is_empty() {
                        let ids = registry.tag_index.query_any(tags);
                        result.extend(ids);
                    }
                },
                MembershipCondition::FilterTagAll { tags } => {
                    if !tags.is_empty() && !result.is_empty() {
                        let ids = registry.tag_index.query_all(tags);
                        result.retain(|id| ids.contains(id));
                    }
                },
                MembershipCondition::FilterTagAny { tags } => {
                    if !tags.is_empty() && !result.is_empty() {
                        let ids = registry.tag_index.query_any(tags);
                        result.retain(|id| ids.contains(id));
                    }
                },
                MembershipCondition::IncludeIds { ids }
                    if !ids.is_empty() => {
                        // 只保留存在的 Id
                        // 直接使用迭代器拓展, 性能更好
                        result.extend(ids.iter().filter(|id| registry.contains_visible(**id)).copied());
                        // TODO[2026-10-03]: 究竟使用 contains_visible (不含遮蔽) 还是 contains_including_shadowed (含遮蔽), 需等待后续决定
                    },
                _ => {},
            }
        }

        // 2. 应用排除型条件, 顺序不影响排除的效果
        for cond in &self.conditions {
            if let MembershipCondition::ExcludeIds { ids } = cond {
                if !ids.is_empty() {
                    result.retain(|id| !ids.contains(id));
                }
            }
        }

        result
    }

    pub fn redirect_card_refs(&mut self, from: GlobalId, to: GlobalId) -> usize {
        self.conditions
            .iter_mut()
            .map(|cond| usize::from(cond.redirect_card_refs(from, to)))
            .sum()
    }
}


/// 声明 `Deck.event_groups` 中每个活动标签组成员卡片的条件.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EventGroupCondition {
    /// 拉取 `members` 中的所有卡片.
    #[serde(rename = "all")]
    All,

    /// 全局拉取 (基于 `members`): 拥有所有指定标签的卡片.
    #[serde(rename = "tag_all")]
    TagAll{ tags: Vec<Tag> },

    /// 全局拉取 (基于 `members`): 拥有任意指定标签的卡片.
    #[serde(rename = "tag_any")]
    TagAny{ tags: Vec<Tag> },

    /// 本地过滤: 保留当前结果中拥有所有指定标签的卡片.
    #[serde(rename = "filter_tag_all")]
    FilterTagAll{ tags: Vec<Tag> },

    /// 本地过滤: 保留当前结果中拥有任意指定标签的卡片.
    #[serde(rename = "filter_tag_any")]
    FilterTagAny{ tags: Vec<Tag> },

    /// 显示添加: 直接添加这些 `global_id` 的卡片.
    #[serde(rename = "include_ids")]
    IncludeIds{ ids: HashSet<GlobalId> },

    /// 排除: 直接排除这些 `global_id` 的卡片.
    #[serde(rename = "exclude_ids")]
    ExcludeIds{ ids: HashSet<GlobalId> },

    /// 暂不支持
    #[serde(rename = "include_groups")]
    IncludeGroups{ groups: Vec<EventTag> },

    /// 暂不支持
    #[serde(rename = "exclude_groups")]
    ExcludeGroups{ groups: Vec<EventTag> },
}

/// 活动标签分组.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EventGroup {
    /// 按顺序应用的条件列表.
    #[serde(default)]
    pub conditions: Vec<EventGroupCondition>
}

impl EventGroupCondition {
    /// 把 `include_ids` / `exclude_ids` 中指向 `from` 的引用替换为 `to`.
    pub fn redirect_card_refs(&mut self, from: GlobalId, to: GlobalId) -> bool {
        let ids = match self {
            EventGroupCondition::IncludeIds { ids } => ids,
            EventGroupCondition::ExcludeIds { ids } => ids,
            _ => return false,
        };

        if ids.remove(&from) {
            ids.insert(to);
            true
        } else {
            false
        }
    }
}

impl EventGroup {
    /// 根据筛选规则计算实际包含的卡片
    pub fn resolve(&self, members: &HashSet<GlobalId>, registry: &CardRegistry) -> HashSet<GlobalId> {
        let mut result = HashSet::new();

        // 1. 同 members 的处理, 但全局拉取需和 members 取交集
        for cond in &self.conditions {
            match cond {
                EventGroupCondition::All => {
                    result.extend(members.clone());
                },
                EventGroupCondition::TagAll { tags } => {
                    if !tags.is_empty() {
                        let ids = registry.tag_index.query_all(tags);
                        // result.retain(|id| ids.contains(id));
                        result.extend(ids.intersection(members).cloned());
                    }
                },
                EventGroupCondition::TagAny { tags } => {
                    if !tags.is_empty() {
                        let ids = registry.tag_index.query_any(tags);
                        // result.retain(|id| ids.contains(id));
                        result.extend(ids.intersection(members).cloned());
                    }
                },
                EventGroupCondition::FilterTagAll { tags } => {
                    if !tags.is_empty() && !result.is_empty() {
                        let ids = registry.tag_index.query_all(tags);
                        result.retain(|id| ids.contains(id));
                    }
                },
                EventGroupCondition::FilterTagAny { tags } => {
                    if !tags.is_empty() && !result.is_empty() {
                        let ids = registry.tag_index.query_any(tags);
                        result.retain(|id| ids.contains(id));
                    }
                },
                EventGroupCondition::IncludeIds { ids } => {
                    if !ids.is_empty() {
                        // 由于 members 中的卡片已确定存在, 活动组只需确定是否存在于 members 中
                        result.extend(ids.iter().filter(|id| members.contains(id)).copied());
                    }
                },
                EventGroupCondition::IncludeGroups { .. } | EventGroupCondition::ExcludeGroups { .. } => {
                    // TODO[2026-08-18]: 未来实现包含/排除其他活动标签组, 并处理循环依赖
                    // 这里可能存在静默错误, 需等待后续 Banner CRUD 时完善
                    tracing::error!(
                        "IncludeGroups 和 ExcludeGroups 筛选条件当前不支持, 已跳过"
                    )
                },
                _ => {},
            }
        }

        // 2. 应用所有排除型条件
        for cond in &self.conditions {
            if let EventGroupCondition::ExcludeIds { ids } = cond {
                if !ids.is_empty() {
                    result.retain(|id| !ids.contains(id));
                }
            }
        }

        result
    }

    /// 把活动组规则中指向 `from` 的卡片引用重写为 `to`.
    /// 
    /// # 返回
    /// 被重写的引用数量.
    pub fn redirect_card_refs(&mut self, from: GlobalId, to: GlobalId) -> usize {
        self.conditions
            .iter_mut()
            .map(|cond| usize::from(cond.redirect_card_refs(from, to)))
            .sum()
    }
}


/// 卡组核心数据 (不包含标签 `Tag`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deck {
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

    /// 卡组名称.
    pub name: LocalizedString,

    /// 资源引用.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assets: Option<DeckAssets>,

    /// 成员规则.
    pub members: Membership,

    /// 活动标签分组映射: `EventTag` -> 该分组下的所有卡片 Id 集合.
    /// 这些卡片必须为 `members` 的子集.
    #[serde(default)]
    pub event_groups: HashMap<EventTag, EventGroup>,
}

impl Deck {
    /// 以默认来源 (`local`) 创建一个空卡组.
    pub fn new(name: LocalizedString) -> Self {
        Self {
            global_id: GlobalId::new(),
            origin: Origin::Local,
            forked_from: None,
            shadowed: true,
            name,
            assets: None,
            members: Membership::default(),
            event_groups: HashMap::new(),
        }
    }

    /// 按给定语言解析卡组显示名, 缺失时回退到默认语言或任意可用语言.
    pub fn display_name(&self, locale: &str) -> Option<&str> {
        self.name.get_or_default_locale(locale)
    }

    /// 根据标签条件查询卡组中匹配的卡片 `global_id`.
    /// 
    /// # 参数
    /// - `registry`: 全局卡片注册表 `CardRegistry`, 用于全局标签索引查询.
    /// - `tags`: 普通标签, 要求卡片必须拥有所有提供的标签.
    /// - `event_tags`: 活动标签, 要求卡片必须处于对应的活动分组中.
    /// 
    /// # 返回
    /// 符合条件的卡片 `global` 列表, 顺序不确定.
    pub fn query_cards(&self, registry: &CardRegistry, tags: &[Tag], event_tags: &[EventTag]) -> Vec<GlobalId> {
        let members = self.members.resolve(registry);

        let mut cards = if tags.is_empty() {
            members.clone()
        } else {
            let ids = registry.tag_index.query_all(tags);
            members.intersection(&ids).cloned().collect()
        };

        for event in event_tags {
            let Some(group) = self.event_groups.get(event) else {
                return Vec::new();
            };
            let group_cards = group.resolve(&members, registry);
            cards.retain(|id| group_cards.contains(id));
            
            // NOTE: group.resolve 在 retain 闭包外, 降低了时间复杂度
            // 当前 query_cards 每次还需进行卡组的解析, 是一种为了保留卡组动态性的简化实现
            // 目前实际卡片不超过 1000 张, 性能可以接收
            // 若未来出现性能瓶颈, 再考虑引入 DeckResolver 服务
        }

        // 被派生版本遮蔽的原对象不参与抽卡与覆盖性校验
        cards.retain(|id| registry.contains_visible(*id));

        cards.into_iter().collect()
    }

   /// 把本卡组中所有显式卡片引用 (`include_ids` / `exclude_ids`) 从 `from` 重写为 `to`.
    /// 
    /// Card 被分叉时使用. 返回被重写的引用数量.
    pub fn redirect_card_refs(&mut self, from: GlobalId, to: GlobalId) -> usize {
        let mut count = self.members.redirect_card_refs(from, to);

        for group in self.event_groups.values_mut() {
            count += group.redirect_card_refs(from, to);
        }

        count
    }

    /// 该卡组是否显式引用指定卡片, 区分 include / exclude.
    ///
    /// 覆盖 `members` 与所有 `event_groups` 中的 `include_ids` / `exclude_ids`.
    pub fn card_refs(&self, card_id: GlobalId) -> CardRefs {
        let mut refs = CardRefs::default();

        for cond in &self.members.conditions {
            match cond {
                MembershipCondition::IncludeIds { ids } if ids.contains(&card_id) => refs.included = true,
                MembershipCondition::ExcludeIds { ids } if ids.contains(&card_id) => refs.excluded = true,
                _ => {}
            }
        }

        for group in self.event_groups.values() {
            for cond in &group.conditions {
                match cond {
                    EventGroupCondition::IncludeIds { ids } if ids.contains(&card_id) => refs.included = true,
                    EventGroupCondition::ExcludeIds { ids } if ids.contains(&card_id) => refs.excluded = true,
                    _ => {}
                }
            }
        }

        refs
    }
}

/// 卡组资产引用.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeckAssets {
    /// 卡组封面.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cover: Option<String>,
}

impl DeckAssets {
    /// 是否所有资产引用都为空.
    pub fn is_empty(&self) -> bool {
        self.cover.is_none()
    }
}

/// 带标签的卡组.
/// 即 `Tagged<Deck>`.
pub type TaggedDeck = Tagged<Deck>;


#[cfg(test)]
mod tests {
    use super::*;

    fn id() -> GlobalId {
        GlobalId::new()
    }

    #[test]
    fn redirect_card_refs_in_members_and_event_groups() {
        let (a, b, c) = (id(), id(), id());

        let mut deck = Deck::new(LocalizedString::single("zh-CN", "测试卡组"));
        deck.members.conditions.push(MembershipCondition::IncludeIds {
            ids: HashSet::from([a, b]),
        });
        deck.members.conditions.push(MembershipCondition::ExcludeIds {
            ids: HashSet::from([c]),
        });
        deck.event_groups.insert(EventTag::up(), EventGroup {
            conditions: vec![EventGroupCondition::IncludeIds {
                ids: HashSet::from([a]),
            }],
        });

        let new_a = id();
        let rewritten = deck.redirect_card_refs(a, new_a);

        assert_eq!(rewritten, 2, "members 与 up 组各应重写一次");
        assert!(matches!(
            &deck.members.conditions[0],
            MembershipCondition::IncludeIds { ids } if ids.contains(&new_a) && !ids.contains(&a)
        ));
        assert!(matches!(
            &deck.event_groups[&EventTag::up()].conditions[0],
            EventGroupCondition::IncludeIds { ids } if ids.contains(&new_a)
        ));
        // 未引用的 id 不受影响
        assert!(matches!(
            &deck.members.conditions[1],
            MembershipCondition::ExcludeIds { ids } if ids.contains(&c)
        ));
    }

    #[test]
    fn redirect_reports_zero_when_not_referenced() {
        let mut deck = Deck::new(LocalizedString::single("zh-CN", "测试卡组"));
        deck.members.conditions.push(MembershipCondition::TagAll {
            tags: vec![Tag::new("game", "genshin")],
        });

        assert_eq!(deck.redirect_card_refs(id(), id()), 0);
    }

    #[test]
    fn optional_fields_are_skipped_when_absent() {
        let deck = Deck::new(LocalizedString::single("zh-CN", "测试卡组"));
        let value: serde_json::Value = serde_json::to_value(&deck).unwrap();

        assert_eq!(value["origin"], "local");
        assert_eq!(value["name"]["zh-CN"], "测试卡组");
        assert_eq!(value["members"]["conditions"].as_array().unwrap().len(), 0);
        assert!(value.get("forked_from").is_none());
        assert!(value.get("assets").is_none());
        assert!(value.get("tags").is_none());
    }
}
