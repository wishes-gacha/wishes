//! `CardManager` 卡片引用查询与删除行为的测试.

use std::collections::HashSet;
use std::sync::Arc;

use wishes_lib::domain::localized_string::LocalizedString;
use wishes_lib::domain::tag::Tagged;
use wishes_lib::utils::data_paths::DataPaths;
use wishes_lib::{
    app::admin::card_manager::CardManager,
    domain::{
        card::Card,
        deck::{Deck, Membership, MembershipCondition},
        ids::GlobalId,
    },
    infrastructure::registry::{CardRegistry, DeckRegistry},
};

/// 创建一张测试卡片并注册, 返回其 `GlobalId`.
fn create_card(registry: &CardRegistry) -> GlobalId {
    let card = Card::new(LocalizedString::single("zh-CN", "Test-Card"));
    let id = card.global_id;
    registry.insert(Tagged::new(card));
    id
}

/// 创建通过 `IncludeIds` 引用指定卡片的测试卡组.
fn create_deck_including(deck_registry: &DeckRegistry, card_ids: Vec<GlobalId>) {
    let mut deck = Deck::new(LocalizedString::single("zh-CN", "Test-Deck"));
    deck.members = Membership {
        conditions: vec![MembershipCondition::IncludeIds {
            ids: card_ids.into_iter().collect::<HashSet<_>>(),
        }],
    };
    deck_registry.insert(Tagged::new(deck));
}

/// 创建使用临时目录的 `CardManager`.
fn create_manager(card_registry: &Arc<CardRegistry>, deck_registry: &Arc<DeckRegistry>) -> CardManager {
    CardManager::new(
        card_registry.clone(),
        deck_registry.clone(),
        DataPaths::new(std::env::temp_dir()),
    )
}

// 被卡组 IncludeIds 引用的卡片: 删除仍然成功 (仅记录警告)
#[test]
fn delete_card_allowed_when_referenced() {
    let card_registry = Arc::new(CardRegistry::new());
    let deck_registry = Arc::new(DeckRegistry::new());

    let card_id = create_card(&card_registry);
    create_deck_including(&deck_registry, vec![card_id]);

    let manager = create_manager(&card_registry, &deck_registry);
    manager.delete_card(card_id).unwrap();

    assert!(card_registry.get_including_shadowed(card_id).is_none(), "被引用的卡片也应允许删除");
}

// 未被任何卡组引用时可以正常删除
#[test]
fn delete_card_allowed_when_not_referenced() {
    let card_registry = Arc::new(CardRegistry::new());
    let deck_registry = Arc::new(DeckRegistry::new());

    let referenced = create_card(&card_registry);
    let free = create_card(&card_registry);
    create_deck_including(&deck_registry, vec![referenced]);

    let manager = create_manager(&card_registry, &deck_registry);
    manager.delete_card(free).unwrap();

    assert!(card_registry.get_including_shadowed(free).is_none(), "未被引用的卡片应被删除");
    assert!(card_registry.get_including_shadowed(referenced).is_some(), "其他卡片不应受影响");
}

// 引用查询: 只返回 IncludeIds 显式引用该卡片的卡组
#[test]
fn referencing_decks_returns_matching_decks() {
    let card_registry = Arc::new(CardRegistry::new());
    let deck_registry = Arc::new(DeckRegistry::new());

    let target = create_card(&card_registry);
    let other = create_card(&card_registry);
    create_deck_including(&deck_registry, vec![target]);
    create_deck_including(&deck_registry, vec![GlobalId::new()]);

    let manager = create_manager(&card_registry, &deck_registry);

    assert_eq!(manager.referencing_decks(target).len(), 1);

    assert!(manager.referencing_decks(other).is_empty(), "未被引用的卡片应返回空列表");
}

// 编辑官方卡片 = 分叉: 原对象保留并被遮蔽, 派生对象指向原对象
#[test]
fn updating_official_card_forks_instead_of_mutating() {
    let card_registry = Arc::new(CardRegistry::new());
    let deck_registry = Arc::new(DeckRegistry::new());

    let mut official = Card::new(LocalizedString::single("zh-CN", "胡桃"));
    official.origin = wishes_lib::domain::origin::Origin::Official;
    let mut official = Tagged::new(official);
    official.add_tag(wishes_lib::domain::tag::Tag::new("game", "genshin"));
    let official_id = official.global_id;
    card_registry.insert(official);

    let manager = create_manager(&card_registry, &deck_registry);
    let derived = manager.update_card(
        official_id,
        LocalizedString::single("zh-CN", "我的胡桃"),
        Vec::new(),
        None,
        None,
    ).unwrap();

    assert_eq!(derived.forked_from, Some(official_id), "派生对象应记录来源");
    assert!(derived.origin.is_local(), "派生对象应为本地来源");
    assert_ne!(derived.global_id, official_id, "派生对象应获得新的 global_id");

    assert!(card_registry.get_including_shadowed(official_id).is_some(), "原官方对象必须保持不变");
    assert!(!card_registry.is_visible(official_id), "原官方对象应被遮蔽");
    assert_eq!(card_registry.all_visible().len(), 1);
}

// 删除派生对象后, 原官方对象重新可见
#[test]
fn deleting_forked_card_restores_official_view() {
    let card_registry = Arc::new(CardRegistry::new());
    let deck_registry = Arc::new(DeckRegistry::new());

    let mut official = Card::new(LocalizedString::single("zh-CN", "胡桃"));
    official.origin = wishes_lib::domain::origin::Origin::Official;
    let official_id = official.global_id;
    card_registry.insert(Tagged::new(official));

    let manager = create_manager(&card_registry, &deck_registry);
    let derived = manager.update_card(
        official_id,
        LocalizedString::single("zh-CN", "我的胡桃"),
        Vec::new(),
        None,
        None,
    ).unwrap();

    manager.delete_card(derived.global_id).unwrap();

    assert!(card_registry.is_visible(official_id), "删除派生对象后原对象应恢复可见");
    assert_eq!(card_registry.all_visible().len(), 1);
}

// 分叉卡片时, 卡组中的显式引用被一次性重写
#[test]
fn forking_card_redirects_deck_references() {
    let card_registry = Arc::new(CardRegistry::new());
    let deck_registry = Arc::new(DeckRegistry::new());

    let dir = std::env::temp_dir().join(format!("wishes-fork-retarget-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(&dir).unwrap();

    let mut official = Card::new(LocalizedString::single("zh-CN", "胡桃"));
    official.origin = wishes_lib::domain::origin::Origin::Official;
    let official_id = official.global_id;
    card_registry.insert(Tagged::new(official));

    let mut deck = Deck::new(LocalizedString::single("zh-CN", "原神胡桃 UP 卡组"));
    deck.members = Membership {
        conditions: vec![MembershipCondition::IncludeIds {
            ids: HashSet::from([official_id]),
        }],
    };
    let deck_id = deck.global_id;

    // 引用重写需要回写文件, 因此这里模拟一个"已加载"的卡组
    let deck_dir = dir.join("local").join("decks");
    std::fs::create_dir_all(&deck_dir).unwrap();
    let deck_path = deck_dir.join(format!("{}.json", deck_id));
    std::fs::write(&deck_path, serde_json::to_string_pretty(&deck).unwrap()).unwrap();
    deck_registry.insert(Tagged::new(deck));
    deck_registry.insert_path(deck_id, deck_path.clone());

    let manager = create_manager(&card_registry, &deck_registry);
    let derived = manager.update_card(
        official_id,
        LocalizedString::single("zh-CN", "我的胡桃"),
        Vec::new(),
        None,
        None,
    ).unwrap();

    let updated = deck_registry.get_including_shadowed(deck_id).unwrap();
    match &updated.members.conditions[0] {
        MembershipCondition::IncludeIds { ids } => {
            assert!(ids.contains(&derived.global_id), "卡组引用应指向派生对象");
            assert!(!ids.contains(&official_id), "卡组不应再引用被遮蔽的原对象");
        }
        other => panic!("成员条件类型不应改变: {:?}", other),
    }

    // 改动应已落盘
    let persisted: Deck = serde_json::from_str(&std::fs::read_to_string(&deck_path).unwrap()).unwrap();
    match &persisted.members.conditions[0] {
        MembershipCondition::IncludeIds { ids } => {
            assert!(ids.contains(&derived.global_id), "卡组文件中的引用也应被重写");
        }
        other => panic!("成员条件类型不应改变: {:?}", other),
    }

    // 引用查询也应指向派生对象
    assert_eq!(manager.referencing_decks(derived.global_id).len(), 1);

    std::fs::remove_dir_all(&dir).ok();
}

// 新建卡片写入 data/local/cards/{global_id}.json
#[test]
fn created_card_is_written_with_global_id_filename() {
    let card_registry = Arc::new(CardRegistry::new());
    let deck_registry = Arc::new(DeckRegistry::new());

    let dir = std::env::temp_dir().join(format!("wishes-card-manager-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(&dir).unwrap();

    let manager = CardManager::new(
        card_registry.clone(),
        deck_registry.clone(),
        DataPaths::new(&dir),
    );

    let mut tags = vec![wishes_lib::domain::tag::Tag::new("game", "genshin")];
    tags.push(wishes_lib::domain::tag::Tag::new("type", "character"));

    let card = manager.create_card(
        LocalizedString::single("zh-CN", "新卡片"),
        tags,
        None,
    ).unwrap();

    let expected = dir.join("local").join("cards").join(format!("{}.json", card.global_id));
    assert!(expected.exists(), "卡片文件应写入 {}", expected.display());

    let saved: Card = serde_json::from_str(&std::fs::read_to_string(&expected).unwrap()).unwrap();
    assert_eq!(saved.global_id, card.global_id);
    assert!(saved.origin.is_local());

    std::fs::remove_dir_all(&dir).ok();
}

// 只读来源的卡片不可删除 (官方/拓展包数据必须保持只读)
#[test]
fn delete_read_only_card_is_rejected() {
    let card_registry = Arc::new(CardRegistry::new());
    let deck_registry = Arc::new(DeckRegistry::new());

    let mut official = Card::new(LocalizedString::single("zh-CN", "胡桃"));
    official.origin = wishes_lib::domain::origin::Origin::Official;
    let official_id = official.global_id;
    card_registry.insert(Tagged::new(official));

    let manager = create_manager(&card_registry, &deck_registry);
    let err = manager.delete_card(official_id).unwrap_err();

    assert!(err.to_string().contains("只读来源"), "错误信息应说明只读来源: {err}");
    assert!(card_registry.get_including_shadowed(official_id).is_some(), "只读卡片不应被删除");
}

// 被遮蔽的卡组仍可能被卡池引用, 卡片分叉时也必须重写其显式引用
#[test]
fn forking_card_redirects_references_in_shadowed_decks() {
    let card_registry = Arc::new(CardRegistry::new());
    let deck_registry = Arc::new(DeckRegistry::new());

    let dir = std::env::temp_dir().join(format!("wishes-shadow-deck-{}", uuid::Uuid::now_v7()));
    let deck_dir = dir.join("local").join("decks");
    std::fs::create_dir_all(&deck_dir).unwrap();

    let mut official = Card::new(LocalizedString::single("zh-CN", "胡桃"));
    official.origin = wishes_lib::domain::origin::Origin::Official;
    let official_id = official.global_id;
    card_registry.insert(Tagged::new(official));

    // 源卡组 (会被派生卡组遮蔽)
    let mut source = Deck::new(LocalizedString::single("zh-CN", "源卡组"));
    source.members = Membership {
        conditions: vec![MembershipCondition::IncludeIds {
            ids: HashSet::from([official_id]),
        }],
    };
    let source_id = source.global_id;
    let source_path = deck_dir.join(format!("{}.json", source_id));
    std::fs::write(&source_path, serde_json::to_string_pretty(&source).unwrap()).unwrap();
    deck_registry.insert(Tagged::new(source.clone()));
    deck_registry.insert_path(source_id, source_path.clone());

    // 派生卡组, 遮蔽源卡组; 模拟"卡池仍引用源卡组"的活跃隐藏卡组
    let mut derived = source.clone();
    derived.global_id = GlobalId::new();
    derived.origin = wishes_lib::domain::origin::Origin::Local;
    derived.forked_from = Some(source_id);
    derived.shadowed = true;
    let derived_id = derived.global_id;
    let derived_path = deck_dir.join(format!("{}.json", derived_id));
    std::fs::write(&derived_path, serde_json::to_string_pretty(&derived).unwrap()).unwrap();
    deck_registry.insert(Tagged::new(derived));
    deck_registry.insert_path(derived_id, derived_path.clone());

    assert!(!deck_registry.is_visible(source_id), "源卡组应被派生卡组遮蔽");
    assert!(deck_registry.is_visible(derived_id));

    let manager = create_manager(&card_registry, &deck_registry);
    let derived_card = manager.update_card(
        official_id,
        LocalizedString::single("zh-CN", "我的胡桃"),
        Vec::new(),
        None,
        None,
    ).unwrap();

    // 隐藏的源卡组也必须重写引用
    let source_updated = deck_registry.get_including_shadowed(source_id).unwrap();
    match &source_updated.members.conditions[0] {
        MembershipCondition::IncludeIds { ids } => {
            assert!(ids.contains(&derived_card.global_id), "隐藏卡组的引用应指向派生卡片");
            assert!(!ids.contains(&official_id), "隐藏卡组不应再引用被遮蔽的原卡片");
        }
        other => panic!("成员条件类型不应改变: {:?}", other),
    }

    let persisted: Deck = serde_json::from_str(&std::fs::read_to_string(&source_path).unwrap()).unwrap();
    match &persisted.members.conditions[0] {
        MembershipCondition::IncludeIds { ids } => {
            assert!(ids.contains(&derived_card.global_id), "隐藏卡组文件中的引用也应被重写");
        }
        other => panic!("成员条件类型不应改变: {:?}", other),
    }

    std::fs::remove_dir_all(&dir).ok();
}
