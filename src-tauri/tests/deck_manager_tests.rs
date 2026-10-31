//! `DeckManager` 的分叉与删除行为测试.

use std::sync::Arc;

use wishes_lib::{
    app::admin::deck_manager::DeckManager,
    domain::{
        deck::Deck,
        localized_string::LocalizedString,
        origin::Origin,
        tag::Tagged,
    },
    infrastructure::registry::{BannerRegistry, CardRegistry, DeckRegistry, LogicRegistry},
    utils::data_paths::DataPaths,
};

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "wishes-deck-manager-{}-{}",
        tag,
        uuid::Uuid::now_v7()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn create_manager(root: &std::path::Path, deck_registry: Arc<DeckRegistry>) -> DeckManager {
    DeckManager::new(
        deck_registry,
        Arc::new(CardRegistry::new()),
        Arc::new(BannerRegistry::new()),
        Arc::new(LogicRegistry::new()),
        DataPaths::new(root),
    )
}

// 同一个只读卡组最多只能派生一次, 避免多个派生版本导致引用混乱
#[test]
fn forking_same_deck_twice_is_rejected() {
    let dir = temp_dir("single-fork");
    let deck_registry = Arc::new(DeckRegistry::new());
    let manager = create_manager(&dir, deck_registry.clone());

    let mut deck = Deck::new(LocalizedString::single("zh-CN", "官方卡组"));
    deck.origin = Origin::Official;
    let source = Tagged::new(deck);
    let source_id = source.global_id;

    let derived = manager.fork_deck_from(&source, |_| {}).unwrap();
    assert_eq!(derived.forked_from, Some(source_id));
    assert!(derived.origin.is_local());
    assert!(derived.shadowed);
    assert_eq!(deck_registry.derived_of(source_id), vec![derived.global_id]);

    let err = manager.fork_deck_from(&source, |_| {}).unwrap_err();
    assert!(err.to_string().contains("已存在派生对象"), "重复派生应被拒绝: {err}");

    std::fs::remove_dir_all(&dir).ok();
}

// 只读来源的卡组不可删除 (官方/拓展包数据必须保持只读)
#[test]
fn delete_read_only_deck_is_rejected() {
    let dir = temp_dir("read-only-delete");
    let deck_registry = Arc::new(DeckRegistry::new());
    let manager = create_manager(&dir, deck_registry.clone());

    let mut deck = Deck::new(LocalizedString::single("zh-CN", "官方卡组"));
    deck.origin = Origin::Official;
    let deck_id = deck.global_id;
    deck_registry.insert(Tagged::new(deck));

    let err = manager.delete_deck(deck_id).unwrap_err();
    assert!(err.to_string().contains("只读来源"), "错误信息应说明只读来源: {err}");
    assert!(
        deck_registry.get_including_shadowed(deck_id).is_some(),
        "只读卡组不应被删除"
    );

    std::fs::remove_dir_all(&dir).ok();
}
