//! `Deck` / `Membership` / `EventGroup` 的条件求值测试.
//! 
//! 数据格式 v2 起, 卡片引用统一为 `GlobalId` (Uuid).

use std::collections::{HashMap, HashSet};

use wishes_lib::{
    domain::{
        card::Card, deck::{Deck, EventGroup, EventGroupCondition, Membership, MembershipCondition, TaggedDeck}, ids::GlobalId, localized_string::LocalizedString, origin::Origin, tag::{EventTag, Tag, Tagged}
    }, infrastructure::registry::CardRegistry
};

/// 在给定注册表中创建带标签的卡片, 并返回它的 `GlobalId`.
fn card_in(registry: &CardRegistry, tags: Vec<(&str, &str)>) -> GlobalId {
    let card = Card::new(LocalizedString::single("zh-CN", "Test-Card"));
    let mut card = Tagged::new(card); 
    for (namespace, value) in tags {
        card.add_tag(Tag::new(namespace, value));
    }
    let id = card.global_id;
    registry.insert(card);
    id
}

/// 创建一张派生卡片 (遮蔽 `source`) 并注册, 返回其 `GlobalId`.
fn derived_card_in(registry: &CardRegistry, source: GlobalId, tags: Vec<(&str, &str)>) -> GlobalId {
    let mut card = Card::new(LocalizedString::single("zh-CN", "Derived-Card"));
    card.origin = Origin::Local;
    card.forked_from = Some(source);
    card.shadowed = true;
    let mut card = Tagged::new(card);
    for (namespace, value) in tags {
        card.add_tag(Tag::new(namespace, value));
    }
    let id = card.global_id;
    registry.insert(card);
    id
}

/// 创建一个空卡组, 返回 `(TaggedDeck, 该卡组使用的注册表)` 之外的成员规则构造辅助.
fn deck_with(name: &str, members: Membership, event_groups: HashMap<EventTag, EventGroup>) -> TaggedDeck {
    let mut deck = Deck::new(LocalizedString::single("zh-CN", name));
    deck.members = members;
    deck.event_groups = event_groups;
    Tagged::new(deck)
}

/// 创建一个卡组成员规则
fn membership(conditions: Vec<MembershipCondition>) -> Membership {
    Membership { conditions }
}

// IncludeIds 包含 Id
#[test]
fn membership_include_ids_only() {
    let registry = CardRegistry::new();
    let a = card_in(&registry, vec![]);
    let b = card_in(&registry, vec![]);

    let rule = membership(vec![MembershipCondition::IncludeIds {
        ids: HashSet::from([a, b]),
    }]);
    let result = rule.resolve(&registry);
    assert_eq!(result, HashSet::from([a, b]));
}

// ExcludeIds 排除 Id
#[test]
fn membership_exclude_ids_only() {
    let registry = CardRegistry::new();
    let a = card_in(&registry, vec![]);

    let rule = membership(vec![MembershipCondition::ExcludeIds {
        ids: HashSet::from([a]),
    }]);
    let result = rule.resolve(&registry);
    assert!(result.is_empty());
}

// TagAll 全局拉取
#[test]
fn membership_tag_all_global() {
    let registry = CardRegistry::new();
    let genshin_char = card_in(&registry, vec![("game", "genshin"), ("type", "character")]);
    let genshin_weapon = card_in(&registry, vec![("game", "genshin"), ("type", "weapon")]);
    card_in(&registry, vec![("game", "starrail"), ("type", "character")]);

    let rule = membership(vec![MembershipCondition::TagAll {
        tags: vec![Tag::new("game", "genshin")],
    }]);
    let result = rule.resolve(&registry);
    assert_eq!(result, HashSet::from([genshin_char, genshin_weapon]));
}

// TagAny 全局拉取
#[test]
fn membership_tag_any_global() {
    let registry = CardRegistry::new();
    let genshin = card_in(&registry, vec![("game", "genshin")]);
    let character = card_in(&registry, vec![("type", "character")]);
    card_in(&registry, vec![("game", "zzz")]);

    let rule = membership(vec![MembershipCondition::TagAny {
        tags: vec![Tag::new("game", "genshin"), Tag::new("type", "character")],
    }]);
    let result = rule.resolve(&registry);
    assert_eq!(result, HashSet::from([genshin, character]));
}

// FilterTagAll 本地过滤
#[test]
fn membership_filter_tag_all() {
    let registry = CardRegistry::new();
    let character = card_in(&registry, vec![("game", "genshin"), ("type", "character")]);
    card_in(&registry, vec![("game", "genshin"), ("type", "weapon")]);

    // 先全局拉取 game=genshin, 再过滤 type=character
    let rule = membership(vec![
        MembershipCondition::TagAll {
            tags: vec![Tag::new("game", "genshin")],
        },
        MembershipCondition::FilterTagAll {
            tags: vec![Tag::new("type", "character")],
        },
    ]);
    let result = rule.resolve(&registry);
    assert_eq!(result, HashSet::from([character]));
}

// FilterTagAny 本地过滤
#[test]
fn membership_filter_tag_any() {
    let registry = CardRegistry::new();
    let genshin = card_in(&registry, vec![("game", "genshin")]);
    let character = card_in(&registry, vec![("type", "character")]);
    card_in(&registry, vec![("game", "zzz")]);

    // 先全局拉取 game=genshin 或 type=character, 再过滤保留 game=genshin 或 type=character (无变化)
    let rule = membership(vec![
        MembershipCondition::TagAny {
            tags: vec![Tag::new("game", "genshin"), Tag::new("type", "character")],
        },
        MembershipCondition::FilterTagAny {
            tags: vec![Tag::new("game", "genshin"), Tag::new("type", "character")],
        },
    ]);
    let result = rule.resolve(&registry);
    assert_eq!(result, HashSet::from([genshin, character]));
}

// 组合: IncludeIds + ExcludeIds
#[test]
fn membership_include_and_exclude() {
    let registry = CardRegistry::new();
    let a = card_in(&registry, vec![]);
    let b = card_in(&registry, vec![]);
    let c = card_in(&registry, vec![]);

    let rule = membership(vec![
        MembershipCondition::IncludeIds {
            ids: HashSet::from([a, b, c]),
        },
        MembershipCondition::ExcludeIds {
            ids: HashSet::from([b]),
        },
    ]);
    let result = rule.resolve(&registry);
    assert_eq!(result, HashSet::from([a, c]));
}

// 顺序敏感: 先 FilterTagAll 再 IncludeIds / 先 IncludeIds 再 FilterTagAll
#[test]
fn membership_order_sensitive() {
    let registry = CardRegistry::new();
    let genshin = card_in(&registry, vec![("game", "genshin")]);
    let zzz = card_in(&registry, vec![("game", "zzz")]);

    // 先本地过滤 (空), 再添加卡片 zzz, 结果 {zzz}
    let rule1 = membership(vec![
        MembershipCondition::FilterTagAll {
            tags: vec![Tag::new("game", "genshin")],
        },
        MembershipCondition::IncludeIds {
            ids: HashSet::from([zzz]),
        },
    ]);
    let result1 = rule1.resolve(&registry);
    assert_eq!(result1, HashSet::from([zzz]));

    // 先添加卡片 zzz, 再本地过滤保留 game:genshin, 由于卡片 zzz 不含该标签, 因此结果为空
    let rule2 = membership(vec![
        MembershipCondition::IncludeIds {
            ids: HashSet::from([zzz]),
        },
        MembershipCondition::FilterTagAll {
            tags: vec![Tag::new("game", "genshin")],
        },
    ]);
    let result2 = rule2.resolve(&registry);
    assert!(result2.is_empty());

    // 参照: 同规则下 genshin 卡片的过滤结果
    let rule3 = membership(vec![
        MembershipCondition::IncludeIds {
            ids: HashSet::from([genshin]),
        },
        MembershipCondition::FilterTagAll {
            tags: vec![Tag::new("game", "genshin")],
        },
    ]);
    assert_eq!(rule3.resolve(&registry), HashSet::from([genshin]));
}

// 复杂组合: TagAny + FilterTagAll + ExcludeIds
#[test]
fn membership_complex() {
    let registry = CardRegistry::new();
    let a = card_in(&registry, vec![("game", "genshin"), ("type", "character")]);
    let b = card_in(&registry, vec![("game", "genshin"), ("type", "weapon")]);
    let c = card_in(&registry, vec![("game", "starrail"), ("type", "character")]);
    let d = card_in(&registry, vec![("game", "zzz"), ("type", "weapon")]);

    let rule = membership(vec![
        MembershipCondition::TagAny {
            tags: vec![Tag::new("game", "genshin"), Tag::new("type", "character")],
        }, // 得到 a,b,c
        MembershipCondition::FilterTagAll {
            tags: vec![Tag::new("type", "character")],
        }, // 过滤保留 a,c
        MembershipCondition::IncludeIds {
            ids: HashSet::from([d]),
        }, // 添加 d => a,c,d
        MembershipCondition::ExcludeIds {
            ids: HashSet::from([c]),
        }, // 移除 c => a,d
    ]);
    let result = rule.resolve(&registry);
    assert_eq!(result, HashSet::from([a, d]));

    // 未使用的变量仅用于说明 b 被过滤掉
    assert!(!result.contains(&b));
}

// EventGroup: All 拉取 members 中所有卡片
#[test]
fn event_group_all() {
    let registry = CardRegistry::new();
    let a = card_in(&registry, vec![]);
    let b = card_in(&registry, vec![]);
    let members = HashSet::from([a, b]);

    let group = EventGroup {
        conditions: vec![EventGroupCondition::All],
    };
    let result = group.resolve(&members, &registry);
    assert_eq!(result, HashSet::from([a, b]));
}

// EventGroup: IncludeIds 包含 Id
#[test]
fn event_group_include_ids_only() {
    let registry = CardRegistry::new();
    let a = card_in(&registry, vec![]);
    let b = card_in(&registry, vec![]);
    let members = HashSet::from([a, b]);

    let group = EventGroup {
        conditions: vec![EventGroupCondition::IncludeIds {
            ids: HashSet::from([a]),
        }],
    };
    let result = group.resolve(&members, &registry);
    assert_eq!(result, HashSet::from([a]));
}

// EventGroup: TagAll
#[test]
fn event_group_tag_all() {
    let registry = CardRegistry::new();
    let five = card_in(&registry, vec![("rarity", "5")]);
    let four = card_in(&registry, vec![("rarity", "4")]);
    card_in(&registry, vec![("rarity", "5")]);      // 不在 members 中
    let members = HashSet::from([five, four]);

    let group = EventGroup {
        conditions: vec![EventGroupCondition::TagAll {
            tags: vec![Tag::new("rarity", "5")],
        }],
    };
    let result = group.resolve(&members, &registry);
    // 全局查得两张 5 星, 但只取与 members 的交集 => five
    assert_eq!(result, HashSet::from([five]));
}

// EventGroup: TagAny
#[test]
fn event_group_tag_any() {
    let registry = CardRegistry::new();
    let genshin = card_in(&registry, vec![("game", "genshin")]);
    card_in(&registry, vec![("type", "character")]);    // 不在 members 中
    let zzz = card_in(&registry, vec![("game", "zzz")]);
    let members = HashSet::from([genshin, zzz]);

    let group = EventGroup {
        conditions: vec![EventGroupCondition::TagAny {
            tags: vec![Tag::new("game", "genshin"), Tag::new("type", "character")],
        }],
    };
    let result = group.resolve(&members, &registry);
    // 全局查得 genshin 与 character, 与 members 交集 => genshin
    assert_eq!(result, HashSet::from([genshin]));
}

// EventGroup: FilterTagAll
#[test]
fn event_group_filter_tag_all() {
    let registry = CardRegistry::new();
    let character = card_in(&registry, vec![("type", "character")]);
    let weapon = card_in(&registry, vec![("type", "weapon")]);
    let members = HashSet::from([character, weapon]);

    // 先拉取全集, 再用 FilterTagAll 筛选
    let group = EventGroup {
        conditions: vec![
            EventGroupCondition::All,
            EventGroupCondition::FilterTagAll {
                tags: vec![Tag::new("type", "character")],
            },
        ],
    };
    let result = group.resolve(&members, &registry);
    assert_eq!(result, HashSet::from([character]));
}

// EventGroup: FilterTagAny
#[test]
fn event_group_filter_tag_any() {
    let registry = CardRegistry::new();
    let genshin = card_in(&registry, vec![("game", "genshin")]);
    let character = card_in(&registry, vec![("type", "character")]);
    let zzz = card_in(&registry, vec![("game", "zzz")]);
    let members = HashSet::from([genshin, character, zzz]);

    let group = EventGroup {
        conditions: vec![
            EventGroupCondition::IncludeIds {
                ids: HashSet::from([genshin, character, zzz]),
            },
            EventGroupCondition::FilterTagAny {
                tags: vec![Tag::new("game", "genshin"), Tag::new("type", "character")],
            },
        ],
    };
    let result = group.resolve(&members, &registry);
    // 过滤保留 genshin 或 character → {genshin, character}
    assert_eq!(result, HashSet::from([genshin, character]));
}

// EventGroup: ExcludeIds
#[test]
fn event_group_exclude_ids() {
    let registry = CardRegistry::new();
    let a = card_in(&registry, vec![]);
    let b = card_in(&registry, vec![]);
    let members = HashSet::from([a, b]);

    let group = EventGroup {
        conditions: vec![
            EventGroupCondition::IncludeIds {
                ids: HashSet::from([a, b]),
            },
            EventGroupCondition::ExcludeIds {
                ids: HashSet::from([a]),
            },
        ],
    };
    let result = group.resolve(&members, &registry);
    assert_eq!(result, HashSet::from([b]));
}

// EventGroup: 复杂组合 (模拟原神 up 组)
#[test]
fn event_group_complex() {
    let registry = CardRegistry::new();
    let c5 = card_in(&registry, vec![("rarity", "5"), ("type", "character")]); // 5 星角色
    let w5 = card_in(&registry, vec![("rarity", "5"), ("type", "weapon")]);    // 5 星武器
    let c4 = card_in(&registry, vec![("rarity", "4"), ("type", "character")]); // 4 星角色
    let w4 = card_in(&registry, vec![("rarity", "4"), ("type", "weapon")]);    // 4 星武器
    let c3 = card_in(&registry, vec![("rarity", "3"), ("type", "character")]); // 3 星角色
    let members = HashSet::from([c5, w5, c4, w4, c3]);

    // up 组: 所有 5 星角色 + 指定的 4 星角色, 但排除武器
    let group = EventGroup {
        conditions: vec![
            EventGroupCondition::TagAll {
                tags: vec![Tag::new("rarity", "5"), Tag::new("type", "character")],
            }, // c5
            EventGroupCondition::IncludeIds {
                ids: HashSet::from([c4]),
            }, // 添加 c4
            EventGroupCondition::ExcludeIds {
                ids: HashSet::from([w5]),
            }, // 排除 w5 (不在结果中, 无害)
        ],
    };
    let result = group.resolve(&members, &registry);
    assert_eq!(result, HashSet::from([c5, c4]));
}

// Deck::query_cards 无活动标签
#[test]
fn deck_query_no_event_tags() {
    let registry = CardRegistry::new();
    let genshin = card_in(&registry, vec![("game", "genshin"), ("rarity", "5")]);
    let zzz = card_in(&registry, vec![("game", "zzz"), ("rarity", "4")]);
    let starrail = card_in(&registry, vec![("game", "starrail"), ("rarity", "5")]);

    let members = membership(vec![MembershipCondition::IncludeIds {
        ids: HashSet::from([genshin, zzz, starrail]),
    }]);

    let deck = deck_with("Test-Deck", members, HashMap::new());

    let tags = vec![Tag::new("game", "genshin")];
    let result = deck.query_cards(&registry, &tags, &[]);
    assert_eq!(result, vec![genshin]);
}

// Deck::query_cards 有活动标签
#[test]
fn deck_query_with_event_tags() {
    let registry = CardRegistry::new();
    let r5 = card_in(&registry, vec![("game", "genshin"), ("rarity", "5")]);
    let r4 = card_in(&registry, vec![("game", "genshin"), ("rarity", "4")]);
    let other = card_in(&registry, vec![("game", "starrail"), ("rarity", "5")]);

    let members = membership(vec![MembershipCondition::IncludeIds {
        ids: HashSet::from([r5, r4, other]),
    }]);

    let mut event_groups = HashMap::new();
    event_groups.insert(EventTag::up(), EventGroup {
        conditions: vec![EventGroupCondition::IncludeIds {
            ids: HashSet::from([r4]),
        }],
    });

    let deck = deck_with("Test-Deck", members, event_groups);

    let tags = vec![Tag::new("game", "genshin")];
    let event_tags = vec![EventTag::up()];
    let result = deck.query_cards(&registry, &tags, &event_tags);
    assert_eq!(result, vec![r4]);
}

// Deck::query_cards 活动组不存在的情况
#[test]
fn deck_query_event_group_missing() {
    let registry = CardRegistry::new();
    let genshin = card_in(&registry, vec![("game", "genshin")]);

    let members = membership(vec![MembershipCondition::IncludeIds {
        ids: HashSet::from([genshin]),
    }]);

    let deck = deck_with("Test-Deck", members, HashMap::new());

    let tags = vec![Tag::new("game", "genshin")];
    let result = deck.query_cards(&registry, &tags, &[EventTag::up()]);
    assert!(result.is_empty());
}

// 完整的原神角色 UP 卡组模拟测试
#[test]
fn deck_query_genshin_character_up_simulation() {
    let registry = CardRegistry::new();
    // 插入模拟卡片
    // c4_const 为常驻, up5 为 UP
    let w3 = card_in(&registry, vec![("game", "genshin"), ("rarity", "3"), ("type", "weapon")]);
    let c4 = card_in(&registry, vec![("game", "genshin"), ("rarity", "4"), ("type", "character")]);
    let w4 = card_in(&registry, vec![("game", "genshin"), ("rarity", "4"), ("type", "weapon")]);
    let c5_const = card_in(&registry, vec![("game", "genshin"), ("rarity", "5"), ("type", "character")]);
    let up5 = card_in(&registry, vec![("game", "genshin"), ("rarity", "5"), ("type", "character")]);
    card_in(&registry, vec![("game", "starrail"), ("rarity", "5"), ("type", "character")]);

    // members: 所有原神角色(3星、4星、5星)
    let members_rule = membership(vec![
        MembershipCondition::TagAll {
            tags: vec![Tag::new("game", "genshin"), Tag::new("rarity", "3")],
        },
        MembershipCondition::TagAll {
            tags: vec![Tag::new("game", "genshin"), Tag::new("rarity", "4")],
        },
        // 添加常驻 5 星和 UP 5 星
        MembershipCondition::IncludeIds {
            ids: HashSet::from([c5_const, up5]),
        },
    ]);

    // up 组: UP 5 星 + 指定 UP 4 星
    let mut event_groups = HashMap::new();
    event_groups.insert(EventTag::up(), EventGroup {
        conditions: vec![
            EventGroupCondition::IncludeIds {
                ids: HashSet::from([up5, c4]),
            },
        ],
    });

    // standard 组: 所有成员 排除 up 组 (这里用排除 Id 模拟, 实际可用 ExcludeGroups 但暂未实现)
    event_groups.insert(EventTag::standard(), EventGroup {
        conditions: vec![
            EventGroupCondition::All,
            EventGroupCondition::ExcludeIds {
                ids: HashSet::from([up5, c4]),
            },
        ],
    });

    let deck = deck_with("Genshin Character UP", members_rule, event_groups);

    // 查询 up 组 (普通标签仅 game=genshin 和 type=character 应在成员中)
    let tags = vec![Tag::new("game", "genshin"), Tag::new("type", "character")];
    let result_up = deck.query_cards(&registry, &tags, &[EventTag::up()]);

    let result_up_set: HashSet<_> = result_up.into_iter().collect();
    assert_eq!(result_up_set, HashSet::from([up5, c4]));

    let result_std = deck.query_cards(&registry, &tags, &[EventTag::standard()]);
    let result_std_set: HashSet<_> = result_std.into_iter().collect();

    // c5_const 是常驻 5 星, 且常驻组里只有它为角色
    assert_eq!(result_std_set, HashSet::from([c5_const]));

    // 用到的武器卡片仅用于说明其被 type=character 过滤掉
    assert!(!result_std_set.contains(&w3) && !result_std_set.contains(&w4));
}

// 被遮蔽的原卡片不应出现在抽卡查询中
#[test]
fn deck_query_skips_shadowed_original() {
    let registry = CardRegistry::new();
    let original = card_in(&registry, vec![("game", "genshin"), ("rarity", "5")]);
    let derived = derived_card_in(&registry, original, vec![("game", "genshin"), ("rarity", "5")]);

    assert!(!registry.is_visible(original));
    assert!(registry.is_visible(derived));

    let deck = deck_with(
        "Test-Deck",
        membership(vec![MembershipCondition::TagAll {
            tags: vec![Tag::new("game", "genshin")],
        }]),
        HashMap::new(),
    );

    assert_eq!(deck.query_cards(&registry, &[], &[]), vec![derived]);
}

// 活动组查询同样不应返回被遮蔽的原卡片
#[test]
fn deck_query_event_group_skips_shadowed_original() {
    let registry = CardRegistry::new();
    let original = card_in(&registry, vec![("game", "genshin"), ("rarity", "5")]);
    let derived = derived_card_in(&registry, original, vec![("game", "genshin"), ("rarity", "5")]);

    let mut event_groups = HashMap::new();
    event_groups.insert(EventTag::up(), EventGroup {
        conditions: vec![EventGroupCondition::All],
    });

    let deck = deck_with(
        "Test-Deck",
        membership(vec![MembershipCondition::TagAll {
            tags: vec![Tag::new("game", "genshin")],
        }]),
        event_groups,
    );

    assert_eq!(deck.query_cards(&registry, &[], &[EventTag::up()]), vec![derived]);
}
