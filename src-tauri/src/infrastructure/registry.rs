//! # 注册器
//! 
//! 实际存储卡片、卡组、卡池和逻辑定义，拥有这些对象的所有权.
//! 每个注册器均内建标签索引以支持快速标签查询.
//! 每个注册器均内建标签索引以支持快速标签查询, 以及**分叉索引**以支撑数据遮蔽.
//! 所有存储均采用并发安全的 `DashMap` 和 `Arc`.
//! 
//! # 数据格式 v2 的变化
//! 
//! - 所有对象以 `GlobalId` 为键, 不再有本地自增 `u64` Id, 也没有 `IdAllocator`.
//! - 新增 `forked_from` 反向索引: 若某对象的派生版本存在, 原对象在列表与
//!   抽卡查询中被**遮蔽** (跳过), 用户只需删除派生对象即可"恢复官方版本".

use std::{collections::HashSet, path::PathBuf, sync::Arc};
use dashmap::DashMap;
use super::tag_index::TagIndex;
use crate::{
    domain::{
        ids::GlobalId, card::TaggedCard, deck::TaggedDeck, banner::TaggedBanner,
        logic::{
            builtins::hardcoded::{
                genshin::GenshinCharacterUpLogic,
                starrail::StarrailCharacterUpLogic
            },
            definition::TaggedLogicDefinition,
            executor::{HardcodedExecutor, RuleExecutor}
        },
        tag::{EventTag, Tag}
    }
};

/// 分叉索引.
/// 
/// 只维护 `source -> derived` 这一方向. 反向关系 (`derived -> source`) 由
/// 领域对象自带的 `forked_from` 字段提供, 因此无需反向索引.
/// 
/// - `derived`: 原对象 `global_id` -> 派生对象 `global_id` 集合.
///   若某原对象存在派生对象, 则该原对象在列表与抽卡查询中被遮蔽.
#[derive(Default)]
pub struct ForkIndex {
    /// 派生关系: 原对象 global_id -> 派生对象 global_id 集合.
    /// 
    /// **与遮蔽无关** —— 只要 `forked_from` 指向该对象就登记, 不管
    /// 派生对象的 `shadowed` 是什么值.
    derived: DashMap<GlobalId, HashSet<GlobalId>>,
}

impl ForkIndex {
    /// 创建空的分叉索引.
    pub fn new() -> Self {
        Self::default()
    }

    /// 登记一条派生关系: `derived_id` 派生自 `source_id`.
    pub fn insert(&self, source_id: GlobalId, derived_id: GlobalId) {
        self.derived.entry(source_id).or_default().insert(derived_id);
    }

    /// 移除一条派生关系 (删除派生对象时调用).
    /// 
    /// 当某个原对象已无任何派生对象时, 该原对象自动重新可见.
    pub fn remove(&self, source_id: GlobalId, derived_id: GlobalId) {
        let should_remove = if let Some(mut entry) = self.derived.get_mut(&source_id) {
            entry.remove(&derived_id);
            entry.is_empty()
        } else {
            false
        };

        if should_remove {
            self.derived.remove(&source_id);
        }
    }
    
    /// 指定原对象的所有派生对象 `global_id`.
    pub fn derived_of(&self, source_id: GlobalId) -> Vec<GlobalId> {
        self.derived
            .get(&source_id)
            .map(|entry| entry.value().iter().copied().collect())
            .unwrap_or_default()
    }

    /// 指定原对象是否已被派生 (不管是否遮蔽).
    pub fn has_derived(&self, source_id: GlobalId) -> bool {
        self.derived.contains_key(&source_id)
    }

    /// 被派生过的原对象总数 (不管是否遮蔽).
    pub fn derived_source_count(&self) -> usize {
        self.derived.len()
    }

    // 被派生过的原对象总数.
    // pub fn shadowed_count(&self) -> usize {
    //     self.derived.len()
    // }
}

/// 内置硬编码执行器名称常量.
/// 与 `LogicVariant::Hardcoded::executor_name` 中的取值一一对应,
/// 避免魔法字符串散落在代码各处.
pub mod builtin_hardcoded_executor {
    pub const GENSHIN_CHARACTER_UP: &str = "genshin_character_up";
    pub const STARRAIL_CHARACTER_UP: &str = "starrail_character_up";
}

/// 通用分叉操作宏.
/// 
/// 为 `CardRegistry` / `DeckRegistry` / `BannerRegistry` / `LogicRegistry` 提供统一的分叉索引操作.
/// 
/// 这四个注册器的存储字段均名为 `storage`, 因此共用同一份实现.
/// `LogicRegistry` 原字段名为 `definitions`, 现统一为 `storage`.
macro_rules! impl_fork_ops {
    ($registry:ty, $object:ty, $verb:literal) => {
        impl $registry {
            /// 登记一条派生关系: `derived_id` 派生自 `source_id`.
            /// 
            /// **无条件登记**, 与该派生对象是否遮蔽原对象无关.
            pub fn mark_forked(&self, source_id: GlobalId, derived_id: GlobalId) {
                self.fork_index.insert(source_id, derived_id);
                tracing::debug!(source = %source_id, derived = %derived_id, concat!($verb, " 派生关系登记"));
            }

            /// 解除一条派生关系 (删除派生对象时调用).
            /// 
            /// 解除后该原对象可再次被派生.
            pub fn unmark_forked(&self, source_id: GlobalId, derived_id: GlobalId) {
                self.fork_index.remove(source_id, derived_id);
                tracing::debug!(source = %source_id, derived = %derived_id, concat!($verb, " 派生关系解除"));
            }

            /// 指定对象是否被其派生对象**遮蔽** (即不可见).
            /// 
            /// 判定方式: 该对象存在派生, 且至少一个派生对象的 `shadowed = true`.
            pub fn is_shadowed(&self, id: GlobalId) -> bool {
                self.fork_index.derived_of(id).iter().any(|derived_id| {
                    self.storage.get(derived_id)
                        .map(|entry| entry.value().shadowed)
                        .unwrap_or(false)
                })
            }

            /// 该对象是否可见 (未被派生版本遮蔽).
            pub fn is_visible(&self, id: GlobalId) -> bool {
                !self.is_shadowed(id)
            }

            /// 指定对象的所有派生版本.
            pub fn derived_of(&self, id: GlobalId) -> Vec<GlobalId> {
                self.fork_index.derived_of(id)
            }

            /// 指定对象是否已被派生 (不管是否遮蔽).
            pub fn has_derived(&self, id: GlobalId) -> bool {
                self.fork_index.has_derived(id)
            }

            /// 获取所有**可见**对象 (跳过被遮蔽的原对象), 顺序不确定.
            pub fn all_visible(&self) -> Vec<Arc<$object>> {
                self.storage
                    .iter()
                    .filter(|entry| !self.is_shadowed(*entry.key()))
                    .map(|entry| entry.value().clone())
                    .collect()
            }

            /// 获取所有对象 (含被遮蔽的原对象), 顺序不确定.
            /// 
            /// 用于管理界面展示"派生自 XXX"、引用完整性检查等场景.
            pub fn all_including_shadowed(&self) -> Vec<Arc<$object>> {
                self.storage.iter().map(|entry| entry.value().clone()).collect()
            }

            /// 对象总数 (**包含被遮蔽的原对象**).
            pub fn count(&self) -> usize {
                self.storage.len()
            }

            /// **可见**对象总数.
            pub fn count_visible(&self) -> usize {
                self.storage
                    .iter()
                    .filter(|entry| !self.is_shadowed(*entry.key()))
                    .count()
            }
        }
    };
}

/// 卡片注册器.
/// 
/// 管理所有 `TaggedCard`.
pub struct CardRegistry {
    /// 实际存储结构.
    storage: DashMap<GlobalId, Arc<TaggedCard>>,

    /// 内建的标签索引.
    pub tag_index: TagIndex<GlobalId>,

    /// 卡片配置文件路径记录.
    paths: DashMap<GlobalId, PathBuf>,

    /// 分叉索引.
    fork_index: ForkIndex,

    /// 引用覆盖层 (内存视图): 被遮蔽的原卡片 -> 当前生效的派生卡片.
    ///
    /// 该映射由分叉/遮蔽状态**自动维护** (见 `insert` / `remove_keep_path`),
    /// 解析卡组中的显式卡片引用时通过 [`CardRegistry::resolve_ref`] 应用.
    /// 持久化由 `OverlayRepository` 负责, 官方 JSON 文件永远不会被改写.
    ref_overrides: DashMap<GlobalId, GlobalId>,
}

impl_fork_ops!(CardRegistry, TaggedCard, "TaggedCard");

impl Default for CardRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CardRegistry {
    /// 创建新卡片注册器.
    pub fn new() -> Self {
        Self {
            storage: DashMap::new(),
            tag_index: TagIndex::new(),
            paths: DashMap::new(),
            fork_index: ForkIndex::new(),
            ref_overrides: DashMap::new(),
        }
    }

    /// 检查指定 `global_id` 的卡片是否存在 (**不区分是否被遮蔽**).
    /// 
    /// 引用解析需要用到被遮蔽的对象 (例如官方 Deck 仍引用官方 Card),
    /// 因此这里按存储判断.
    pub fn contains_including_shadowed(&self, id: GlobalId) -> bool {
        self.storage.contains_key(&id)
    }

    /// 检查指定 `global_id` 的卡片是否存在且可见.
    pub fn contains_visible(&self, id: GlobalId) -> bool {
        self.storage.contains_key(&id) && !self.is_shadowed(id)
    }

    /// 插入一张卡片, 同时加入标签索引.
    ///
    /// 若已存在同 `global_id` 对象, 会先清理旧的标签索引和分叉关系.
    /// 引用覆盖层随 `forked_from` / `shadowed` 自动更新:
    /// 遮蔽原对象的派生卡片会登记 `原对象 -> 派生对象` 的覆盖.
    pub fn insert(&self, card: TaggedCard) {
        let id = card.global_id;
        let new_forked_from = card.forked_from;
        let new_shadowed = card.shadowed;

        // 检查旧的标签索引, 同时取出旧的 forked_from
        let old_forked_from = if let Some(old) = self.storage.get(&id) {
            for tag in &old.tags {
                self.tag_index.remove(id, tag);
            }
            old.forked_from
        } else {
            None
        };

        // 无条件解除旧的遮蔽关系, 若 shadow 为 true, 之后会重新建立
        if let Some(source) = old_forked_from {
            self.unmark_forked(source, id);
            self.clear_ref_override_if_target(source, id);
        }

        let tags: Vec<Tag> = card.tags.iter().cloned().collect();
        self.tag_index.insert(id, &tags);
        self.storage.insert(id, Arc::new(card));

        // 无条件登记派生关系 (不管 shadowed 值)
        if let Some(source) = new_forked_from {
            self.mark_forked(source, id);
            // 遮蔽原对象时, 引用覆盖层把对原对象的引用解析到派生对象
            if new_shadowed {
                self.ref_overrides.insert(source, id);
            }
        }
    }

    /// 记录卡片配置文件的路径.
    pub fn insert_path(&self, id: GlobalId, path: PathBuf) {
        self.paths.insert(id, path);
    }

    /// 删除指定 `global_id` 的卡片解除其分叉关系, **保留**路径记录.
    /// 
    /// 返回被删除的卡片 (若存在).
    pub fn remove_keep_path(&self, id: GlobalId) -> Option<Arc<TaggedCard>> {
        let entry = self.storage.remove(&id)?;
        let card = entry.1;

        for tag in &card.tags {
            self.tag_index.remove(id, tag);
        }

        if let Some(source) = card.forked_from {
            self.unmark_forked(source, id);
            self.clear_ref_override_if_target(source, id);
        }

        Some(card)
    }

    /// 删除指定 `global_id` 的卡片并解除其分叉关系, 同时**清理**路径记录.
    /// 
    /// 相当于先删除路径记录, 再调用 `remove_keep_path`.
    /// 
    /// 返回被删除的卡片 (若存在)
    pub fn remove_with_path(&self, id: GlobalId) -> Option<Arc<TaggedCard>> {
        self.paths.remove(&id);
        self.remove_keep_path(id)
    }

    /// 删除指定 `global_id` 的卡片配置文件路径的存储.
    pub fn remove_path(&self, id: GlobalId) {
        self.paths.remove(&id);
    }

    /// 通过 `global_id` 获取卡片的一个 `Arc` 引用 (**不区分是否被遮蔽**).
    pub fn get_including_shadowed(&self, id: GlobalId) -> Option<Arc<TaggedCard>> {
        self.storage.get(&id).map(|refs| refs.clone())
    }
    
    /// 通过 `global_id` 获取卡片的一个 `Arc` 引用, 仅在该卡片**可见**时返回.
    pub fn get_visible(&self, id: GlobalId) -> Option<Arc<TaggedCard>> {
        if self.is_shadowed(id) {
            None
        } else {
            self.storage.get(&id).map(|entry| entry.value().clone())
        }
    }

    /// 通过 `global_id` 获取对应卡片的配置文件路径.
    pub fn get_path(&self, id: GlobalId) -> Option<PathBuf> {
        self.paths.get(&id).map(|refs| refs.clone())
    }

    /// 解析卡片引用在当前**生效**对象上的 `global_id`.
    ///
    /// 若 `id` 是被遮蔽的原卡片, 返回遮蔽它的派生卡片; 否则原样返回.
    /// 卡组中的 `include_ids` / `exclude_ids` 在求值时统一经过此方法,
    /// 从而无需改写任何卡组文件即可让分叉立即生效.
    pub fn resolve_ref(&self, id: GlobalId) -> GlobalId {
        // 单派生约束下至多一跳; 循环上限仅用于防御脏数据造成的环
        const MAX_DEPTH: usize = 8;

        let mut current = id;
        for _ in 0..MAX_DEPTH {
            match self.ref_overrides.get(&current).map(|entry| *entry.value()) {
                Some(next) if next != current => current = next,
                _ => break,
            }
        }
        current
    }

    /// 指定卡片当前的引用覆盖目标 (若存在).
    pub fn ref_override_of(&self, id: GlobalId) -> Option<GlobalId> {
        self.ref_overrides.get(&id).map(|entry| *entry.value())
    }

    /// 当前登记的全部引用覆盖 `(原对象, 生效派生对象)`, 按 Id 排序.
    ///
    /// 用于同步到 `OverlayRepository` 持久化.
    pub fn ref_overrides_snapshot(&self) -> Vec<(GlobalId, GlobalId)> {
        let mut entries: Vec<(GlobalId, GlobalId)> = self
            .ref_overrides
            .iter()
            .map(|entry| (*entry.key(), *entry.value()))
            .collect();
        entries.sort_by_key(|(from, to)| (*from, *to));
        entries
    }

    /// 若 `source` 的覆盖目标恰为 `target`, 移除该覆盖记录.
    ///
    /// 用于删除或取消遮蔽派生对象: 不影响指向其他派生对象的覆盖.
    fn clear_ref_override_if_target(&self, source: GlobalId, target: GlobalId) {
        self.ref_overrides.remove_if(&source, |_, to| *to == target);
    }
}

/// 卡组注册器.
/// 
/// 管理所有 `TaggedDeck`.
pub struct DeckRegistry {
    /// 实际存储结构.
    storage: DashMap<GlobalId, Arc<TaggedDeck>>,

    /// 标签索引.
    pub tag_index: TagIndex<GlobalId>,

    /// 卡组配置文件路径记录.
    paths: DashMap<GlobalId, PathBuf>,

    /// 分叉索引.
    fork_index: ForkIndex,
}

impl_fork_ops!(DeckRegistry, TaggedDeck, "TaggedDeck");

impl Default for DeckRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl DeckRegistry {
    /// 创建新的卡组注册器.
    pub fn new() -> Self {
        Self {
            storage: DashMap::new(),
            tag_index: TagIndex::new(),
            paths: DashMap::new(),
            fork_index: ForkIndex::new(),
        }
    }

    /// 检查指定 `global_id` 的卡组是否存在 (**不区分是否被遮蔽**).
    pub fn contains_including_shadowed(&self, id: GlobalId) -> bool {
        self.storage.contains_key(&id)
    }

    /// 检查指定 `global_id` 的卡组是否存在且可见.
    pub fn contains_visible(&self, id: GlobalId) -> bool {
        self.storage.contains_key(&id) && !self.is_shadowed(id)
    }

    /// 插入一个卡组.
    pub fn insert(&self, deck: TaggedDeck) {
        let id = deck.global_id;
        let new_forked_from = deck.forked_from;

        let old_forked_from = if let Some(old) = self.storage.get(&id) {
            for tag in &old.tags {
                self.tag_index.remove(id, tag);
            }
            old.forked_from
        } else {
            None
        };

        if let Some(source) = old_forked_from {
            if Some(source) != new_forked_from {
                self.unmark_forked(source, id);
            }
        }

        let tags: Vec<Tag> = deck.tags.iter().cloned().collect();
        self.tag_index.insert(id, &tags);
        self.storage.insert(id, Arc::new(deck));

        if let Some(source) = new_forked_from {
            self.mark_forked(source, id);
        }
    }

    /// 记录卡组配置文件的路径.
    pub fn insert_path(&self, id: GlobalId, path: PathBuf) {
        self.paths.insert(id, path);
    }

    /// 删除指定 `global_id` 的卡组, **保留**路径记录.
    /// 
    /// 返回被删除的卡组 (若存在).
    pub fn remove_keep_path(&self, id: GlobalId) -> Option<Arc<TaggedDeck>> {
        let entry = self.storage.remove(&id)?;
        let deck = entry.1;

        for tag in &deck.tags {
            self.tag_index.remove(id, tag);
        }

        if let Some(source) = deck.forked_from {
            self.unmark_forked(source, id);
        }

        Some(deck)
    }

    /// 删除指定 `global_id` 的卡组, 同时**清理**路径记录.
    /// 
    /// 相当于先删除路径记录, 再调用 `remove_keep_path`.
    /// 
    /// 返回被删除的卡组 (若存在).
    pub fn remove_with_path(&self, id: GlobalId) -> Option<Arc<TaggedDeck>> {
        self.paths.remove(&id);
        self.remove_keep_path(id)
    }

    /// 删除指定 `global_id` 的卡组配置文件路径的存储.
    pub fn remove_path(&self, id: GlobalId) {
        self.paths.remove(&id);
    }

     /// 通过 `global_id` 获取卡组的一个 `Arc` 引用 (**不区分是否被遮蔽**).
    pub fn get_including_shadowed(&self, id: GlobalId) -> Option<Arc<TaggedDeck>> {
        self.storage.get(&id).map(|entry| entry.clone())
    }

    /// 通过 `global_id` 获取卡组的 `Arc` 引用, 仅在**可见**时返回.
    pub fn get_visible(&self, id: GlobalId) -> Option<Arc<TaggedDeck>> {
        if self.is_shadowed(id) {
            None
        } else {
            self.storage.get(&id).map(|entry| entry.clone())
        }
    }

    /// 通过 `global_id` 获取对应卡组的配置文件路径.
    pub fn get_path(&self, id: GlobalId) -> Option<PathBuf> {
        self.paths.get(&id).map(|p| p.clone())
    }

    /// 由卡片 `global_id` 反查显式引用它的可见卡组.
    /// 
    /// 仅统计 `members` / `event_groups` 中通过 `include_ids` 显式引用的卡片;
    /// 通过标签规则动态包含的卡片不计入.
    pub fn decks_referencing_card(&self, card_id: GlobalId) -> Vec<Arc<TaggedDeck>> {
        self.all_visible()
            .into_iter()
            .filter(|deck| deck.card_refs(card_id).is_any())
            .collect()
    }
}

/// 卡池注册器.
/// 
/// 管理所有 `TaggedBanner`.
pub struct BannerRegistry {
    /// 实际存储结构.
    storage: DashMap<GlobalId, Arc<TaggedBanner>>,

    /// 标签索引.
    pub tag_index: TagIndex<GlobalId>,

    /// 卡组 -> 引用它的卡池 反向索引.
    deck_to_banners: DashMap<GlobalId, HashSet<GlobalId>>,

    /// 卡池配置文件路径记录
    paths: DashMap<GlobalId, PathBuf>,

    fork_index: ForkIndex,

    // TODO[2026-10-03]: logic_to_banners 反向索引等待 Logic CRUD 时完善.
}

impl_fork_ops!(BannerRegistry, TaggedBanner, "TaggedBanner");

impl Default for BannerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl BannerRegistry {
    /// 创建新的卡池注册器.
    pub fn new() -> Self {
        Self {
            storage: DashMap::new(),
            tag_index: TagIndex::new(),
            deck_to_banners: DashMap::new(),
            paths: DashMap::new(),
            fork_index: ForkIndex::new(),
        }
    }

    /// 检查指定 `global_id` 的卡池是否存在 (**不区分是否被遮蔽**).
    pub fn contains_including_shadowed(&self, id: GlobalId) -> bool {
        self.storage.contains_key(&id)
    }

    /// 检查指定 `global_id` 的卡池是否存在且可见.
    pub fn contains_visible(&self, id: GlobalId) -> bool {
        self.storage.contains_key(&id) && !self.is_shadowed(id)
    }

    /// 插入一个新卡池, 同时维护标签, 分叉与 `deck_to_banners` 索引.
    /// 
    /// 若存在同 `global_id` 对象且 `deck_id` 发生变化, 会清理旧的
    /// `deck_to_banners` 映射.
    pub fn insert(&self, banner: TaggedBanner) {
        let id = banner.global_id;
        let new_deck_id = banner.deck_id;
        let new_forked_from = banner.forked_from;

        let (old_deck_id_opt, old_forked_from) = if let Some(old) = self.storage.get(&id) {
            for tag in &old.tags {
                self.tag_index.remove(id, tag);
            }
            (Some(old.deck_id), old.forked_from)
        } else {
            (None, None)
        };

        if let Some(old_deck_id) = old_deck_id_opt {
            if old_deck_id != new_deck_id {
                if let Some(mut entry) = self.deck_to_banners.get_mut(&old_deck_id) {
                    entry.remove(&id);
                    if entry.is_empty() {
                        drop(entry);
                        self.deck_to_banners.remove(&old_deck_id);
                    }
                }
            }
        }

        if let Some(source) = old_forked_from {
            if Some(source) != new_forked_from {
                self.unmark_forked(source, id);
            }
        }

        self.deck_to_banners
            .entry(new_deck_id)
            .or_default()
            .insert(id);

        let tags: Vec<Tag> = banner.tags.iter().cloned().collect();
        self.tag_index.insert(id, &tags);
        self.storage.insert(id, Arc::new(banner));

        if let Some(source) = new_forked_from {
            self.mark_forked(source, id);
        }
    }

    /// 记录卡池配置文件的路径.
    pub fn insert_path(&self, id: GlobalId, path: PathBuf) {
        self.paths.insert(id, path);
    }

    /// 删除指定 `global_id` 的卡池, **保留**路径记录.
    /// 
    /// 同时清理标签索引, `deck_to_banners` 反向索引与分叉关系.
    /// 
    /// 返回被删除的卡池 (若存在).
    pub fn remove_keep_path(&self, id: GlobalId) -> Option<Arc<TaggedBanner>> {
        let entry = self.storage.remove(&id)?;
        let banner = entry.1;

        for tag in &banner.tags {
            self.tag_index.remove(id, tag);
        }

        if let Some(mut entry) = self.deck_to_banners.get_mut(&banner.deck_id) {
            entry.remove(&id);
            if entry.is_empty() {
                drop(entry);
                self.deck_to_banners.remove(&banner.deck_id);
            }
        }

        if let Some(source) = banner.forked_from {
            self.unmark_forked(source, id);
        }

        Some(banner)
    }

    /// 删除指定 `global_id` 的卡池, 同时**清理**路径记录.
    /// 
    /// 相当于先删除路径记录, 再调用 `remove_keep_path`.
    /// 
    /// 返回被删除的卡池 (若存在).
    pub fn remove_with_path(&self, id: GlobalId) -> Option<Arc<TaggedBanner>> {
        self.paths.remove(&id);
        self.remove_keep_path(id)
    }

    /// 删除指定 `global_id` 的卡池配置文件路径的存储.
    pub fn remove_path(&self, id: GlobalId) {
        self.paths.remove(&id);
    }

    /// 查找引用某个卡组的卡池的 `global_id`.
    /// 
    /// 返回的是当前存储中存在的卡池 (**含被遮蔽对象**).
    pub fn find_banners_by_deck(&self, deck_id: GlobalId) -> Vec<GlobalId> {
        self.deck_to_banners
            .get(&deck_id)
            .map(|entry| entry.value().iter().copied().collect())
            .unwrap_or_default()
    }

    /// 通过 `global_id` 获取卡池的 `Arc<_>` 引用 (**不区分是否被遮蔽**).
    pub fn get_including_shadowed(&self, id: GlobalId) -> Option<Arc<TaggedBanner>> {
        self.storage.get(&id).map(|entry| entry.clone())
    }

    /// 通过 `global_id` 获取卡池的 `Arc` 引用, 仅在**可见**时返回.
    pub fn get_visible(&self, id: GlobalId) -> Option<Arc<TaggedBanner>> {
        if self.is_shadowed(id) {
            None
        } else {
            self.storage.get(&id).map(|entry| entry.value().clone())
        }
    }

    /// 通过 `global_id` 获取对应卡池的配置文件路径.
    pub fn get_path(&self, id: GlobalId) -> Option<PathBuf> {
        self.paths.get(&id).map(|p| p.clone())
    }

    /// 获取所有卡池 `global_id` 的列表 (**含被遮蔽对象**), 顺序不确定.
    pub fn all_ids_including_shadowed(&self) -> Vec<GlobalId> {
        self.storage.iter().map(|entry| *entry.key()).collect()
    }

    /// 获取所有**可见**卡池 `global_id` 的列表, 顺序不确定.
    pub fn all_visible_ids(&self) -> Vec<GlobalId> {
        self.storage
            .iter()
            .filter(|entry| !self.is_shadowed(*entry.key()))
            .map(|entry| *entry.key())
            .collect()
    }
}

/// 逻辑定义与执行器注册器.
/// 
/// 维护逻辑定义 `TaggedLogicDefinition` 以及两类执行器:
/// - 硬编码执行器 `HardcodedExecutor`
/// - 规则执行器 `RuleExecutor`
pub struct LogicRegistry {
    /// **逻辑定义**的实际存储结构.
    /// 
    /// 为保持与其他注册器统一, 采用 `storage` 作为字段名.
    storage: DashMap<GlobalId, Arc<TaggedLogicDefinition>>,
    
    /// 硬编码执行器的实际存储结构.
    hardcoded_executors: DashMap<String, Arc<dyn HardcodedExecutor>>,
    
    /// 规则执行器的实际存储结构.
    rule_executors: DashMap<String, Arc<dyn RuleExecutor>>,

    /// 逻辑定义的标签索引.
    pub tag_index: TagIndex<GlobalId>,

    /// 分叉索引.
    fork_index: ForkIndex,
}

impl_fork_ops!(LogicRegistry, TaggedLogicDefinition, "TaggedLogicDefinition");

impl Default for LogicRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl LogicRegistry {
    /// 创建新的逻辑注册器
    /// 同时注册所有内置逻辑执行器.
    pub fn new() -> Self {
        // 注册内置执行器
        let registry = Self {
            storage: DashMap::new(),
            hardcoded_executors: DashMap::new(),
            rule_executors: DashMap::new(),
            tag_index: TagIndex::new(),
            fork_index: ForkIndex::new(),
        };

        registry.register_builtin();
        registry
    }

    /// 注册内置逻辑执行器
    fn register_builtin(&self) {
        self.hardcoded_executors.insert(
            builtin_hardcoded_executor::GENSHIN_CHARACTER_UP.into(),
            Arc::new(GenshinCharacterUpLogic)
        );
        self.hardcoded_executors.insert(
            builtin_hardcoded_executor::STARRAIL_CHARACTER_UP.into(),
            Arc::new(StarrailCharacterUpLogic)
        );
    }

    // ----- 领域语义别名 -----

    /// 获取所有**可见**逻辑定义, 顺序不确定.
    pub fn all_visible_definitions(&self) -> Vec<Arc<TaggedLogicDefinition>> {
        self.all_visible()
    }

    /// 获取所有逻辑定义 (含被遮蔽的原对象), 顺序不确定.
    pub fn all_definitions_including_shadowed(&self) -> Vec<Arc<TaggedLogicDefinition>> {
        self.all_including_shadowed()
    }

    /// 逻辑定义总数 (**包含被遮蔽的原对象**).
    pub fn count_definitions(&self) -> usize {
        self.count()
    }

    /// **可见**逻辑定义总数.
    pub fn count_visible_definitions(&self) -> usize {
        self.count_visible()
    }

    /// 检查是否存在指定 `global_id` 的逻辑定义 (**不区分是否被遮蔽**).
    pub fn contains_definition(&self, id: GlobalId) -> bool {
        self.storage.contains_key(&id)
    }

    // ----- CRUD -----
    // TODO[2026-10-03]: 此处 CRUD 不完善, 等待后续 Logic CRUD.

    /// 检查是否存在指定 `global_id` 的逻辑定义 (**不区分是否被遮蔽**).
    pub fn contains_definition_including_shadowed(&self, id: GlobalId) -> bool {
        self.storage.contains_key(&id)
    }

    /// 检查是否存在指定 `global_id` 的逻辑定义且可见.
    pub fn contains_definition_visible(&self, id: GlobalId) -> bool {
        self.storage.contains_key(&id) && !self.is_shadowed(id)
    }

    pub fn insert_definition(&self, def: TaggedLogicDefinition) {
        let id = def.global_id;
        let new_forked_from = def.forked_from;

        let old_forked_from = if let Some(old) = self.storage.get(&id) {
            for tag in &old.tags {
                self.tag_index.remove(id, tag);
            }
            old.forked_from
        } else {
            None
        };

        if let Some(source) = old_forked_from {
            if Some(source) != new_forked_from {
                self.unmark_forked(source, id);
            }
        }

        let tags: Vec<Tag> = def.tags.iter().cloned().collect();
        self.tag_index.insert(id, &tags);
        self.storage.insert(id, Arc::new(def));

        if let Some(source) = new_forked_from {
            self.mark_forked(source, id);
        }
    }

    /// 删除指定 `global_id` 的逻辑定义.
    /// 
    /// 返回被删除的定义 (若存在).
    pub fn remove_definition(&self, id: GlobalId) -> Option<Arc<TaggedLogicDefinition>> {
        let entry = self.storage.remove(&id)?;
        let def = entry.1;

        for tag in &def.tags {
            self.tag_index.remove(id, tag);
        }

        if let Some(source) = def.forked_from {
            self.unmark_forked(source, id);
        }

        Some(def)
    }

    /// 通过 `global_id` 获取逻辑定义的 `Arc` 引用 (**不区分是否被遮蔽**).
    /// 
    /// 被遮蔽的定义仍需可解析, 官方卡池仍引用官方逻辑.
    pub fn get_definition_including_shadowed(&self, id: GlobalId) -> Option<Arc<TaggedLogicDefinition>> {
        self.storage.get(&id).map(|entry| entry.value().clone())
    }

    /// 通过 `global_id` 获取逻辑定义的 `Arc` 引用, 仅在**可见**时返回.
    pub fn get_definition_visible(&self, id: GlobalId) -> Option<Arc<TaggedLogicDefinition>> {
        if self.is_shadowed(id) {
            None
        } else {
            self.storage.get(&id).map(|entry| entry.value().clone())
        }
    }

    // ----- 执行器 -----

    /// 检查是否存在指定名称的硬编码执行器.
    pub fn contains_hardcoded_executor(&self, name: &str) -> bool {
        self.hardcoded_executors.contains_key(name)
    }

    /// 检查是否存在指定名称的规则执行器.
    pub fn contains_rule_executor(&self, name: &str) -> bool {
        self.rule_executors.contains_key(name)
    }

    /// 通过名称获取硬编码执行器的 `Arc` 引用.
    pub fn get_hardcoded_executor(&self, name: &str) -> Option<Arc<dyn HardcodedExecutor>> {
        self.hardcoded_executors.get(name).map(|entry| entry.value().clone())
    }

    /// 通过名称获取规则执行器的 `Arc` 引用.
    pub fn get_rule_executor(&self, name: &str) -> Option<Arc<dyn RuleExecutor>> {
        self.rule_executors.get(name).map(|entry| entry.value().clone())
    }

    /// 获取指定硬编码执行器可能输出的所有标签组合 (用于加载器进行标签覆盖性测试).
    pub fn hardcoded_possible_output_combinations(&self, name: &str) -> Option<Vec<(Vec<Tag>, Vec<EventTag>)>> {
        self.hardcoded_executors
            .get(name)
            .map(|guard| guard.value().possible_output_combinations())
    }
}


#[cfg(test)]
mod tests {
    use crate::domain::{card::Card, deck::Deck, localized_string::LocalizedString, origin::Origin, tag::Tagged};

use super::*;
    
    fn card_with_tag(content: &str) -> TaggedCard {
        let card = Card::new(LocalizedString::single("zh-CN", content));
        let mut card = Tagged::new(card);
        card.add_tag(Tag::new("game", "genshin"));
        card
    }

    #[test]
    fn registry_keys_by_global_id() {
        let registry = CardRegistry::new();
        let card = card_with_tag("胡桃");
        let id = card.global_id;

        registry.insert(card);

        assert!(registry.contains_including_shadowed(id));
        assert_eq!(registry.count(), 1);
        assert_eq!(registry.get_including_shadowed(id).unwrap().global_id, id);
        assert_eq!(registry.tag_index.query_all(&[Tag::new("game", "genshin")]).len(), 1);
    }

    #[test]
    fn shadowing_hides_source_but_keeps_it_resolvable() {
        let registry = CardRegistry::new();
        let official = card_with_tag("胡桃");
        let source_id = official.global_id;
        registry.insert(official);

        let mut derived = card_with_tag("我的胡桃");
        derived.origin = Origin::Local;
        derived.forked_from = Some(source_id);
        let derived_id = derived.global_id;
        registry.insert(derived);

        // 列表只看到派生版本, 但按 Id 仍能解析原对象
        assert_eq!(registry.all_visible().len(), 1);
        assert_eq!(registry.all_visible()[0].global_id, derived_id);
        assert!(registry.get_including_shadowed(source_id).is_some(), "被遮蔽的对象仍须可解析");
        assert!(!registry.is_visible(source_id));
        assert_eq!(registry.derived_of(source_id), vec![derived_id]);
    }

    #[test]
    fn deleting_derived_object_restores_source() {
        let registry = CardRegistry::new();
        let source = card_with_tag("胡桃");
        let source_id = source.global_id;
        registry.insert(source);

        let mut derived = card_with_tag("胡桃(派生)");
        derived.forked_from = Some(source_id);
        let derived_id = derived.global_id;
        registry.insert(derived);

        registry.remove_keep_path(derived_id);

        assert!(registry.is_visible(source_id), "删除派生对象后原对象应重新出现");
        assert_eq!(registry.all_visible().len(), 1);
        assert_eq!(registry.all_visible()[0].global_id, source_id);
    }

    #[test]
    fn shadowed_fork_registers_and_removes_ref_override() {
        let registry = CardRegistry::new();
        let source = card_with_tag("胡桃");
        let source_id = source.global_id;
        registry.insert(source);

        let mut derived = card_with_tag("我的胡桃");
        derived.forked_from = Some(source_id);
        derived.shadowed = true;
        let derived_id = derived.global_id;
        registry.insert(derived);

        assert_eq!(registry.resolve_ref(source_id), derived_id);
        assert_eq!(registry.ref_override_of(source_id), Some(derived_id));
        assert_eq!(registry.ref_overrides_snapshot(), vec![(source_id, derived_id)]);
        assert_eq!(registry.resolve_ref(derived_id), derived_id, "派生对象自身不应被覆盖");

        registry.remove_keep_path(derived_id);
        assert_eq!(registry.resolve_ref(source_id), source_id, "删除派生对象后覆盖应解除");
        assert!(registry.ref_overrides_snapshot().is_empty());
    }

    #[test]
    fn unshadowed_fork_does_not_override_references() {
        let registry = CardRegistry::new();
        let source = card_with_tag("胡桃");
        let source_id = source.global_id;
        registry.insert(source);

        let mut derived = card_with_tag("我的胡桃");
        derived.forked_from = Some(source_id);
        derived.shadowed = false;
        registry.insert(derived);

        assert_eq!(registry.resolve_ref(source_id), source_id);
        assert!(registry.ref_override_of(source_id).is_none());
    }

    #[test]
    fn toggling_shadow_updates_ref_override() {
        let registry = CardRegistry::new();
        let source = card_with_tag("胡桃");
        let source_id = source.global_id;
        registry.insert(source);

        let mut derived = card_with_tag("我的胡桃");
        derived.forked_from = Some(source_id);
        derived.shadowed = true;
        let derived_id = derived.global_id;
        registry.insert(derived.clone());

        // 取消遮蔽: 覆盖解除
        let mut unshadowed = derived.clone();
        unshadowed.shadowed = false;
        registry.insert(unshadowed);
        assert_eq!(registry.resolve_ref(source_id), source_id);

        // 重新遮蔽: 覆盖恢复
        let mut shadowed = derived.clone();
        shadowed.shadowed = true;
        registry.insert(shadowed);
        assert_eq!(registry.resolve_ref(source_id), derived_id);
    }

    #[test]
    fn detects_decks_referencing_a_card() {
        let cards = CardRegistry::new();
        let card = card_with_tag("胡桃");
        let card_id = card.global_id;
        cards.insert(card);

        let decks = DeckRegistry::new();
        let mut deck = Deck::new(LocalizedString::single("zh-CN", "卡组"));
        deck.members.conditions.push(
            crate::domain::deck::MembershipCondition::IncludeIds {
                ids: HashSet::from([card_id]),
            },
        );
        let deck_id = deck.global_id;
        let deck = Tagged::new(deck);
        decks.insert(deck);

        let found: Vec<GlobalId> = decks
            .decks_referencing_card(card_id)
            .iter()
            .map(|d| d.global_id)
            .collect();
        assert_eq!(found, vec![deck_id]);
    }
}