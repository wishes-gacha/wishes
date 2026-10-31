//! # 卡组管理器
//! 
//! 管理卡组的创建、查询、更新、删除, 同时管理卡组文件.
//! 
//! # 数据格式 v2 的行为变化
//! 
//! - 新卡组直接落盘到 `data/local/decks/{global_id}.json`, 不再需要 Id 分配器.
//! - 编辑一个**只读来源** (官方 / 拓展包) 的卡组 = **分叉**: 新建派生对象.
//!   注意卡池 (`Banner.deck_id`) 对卡组的引用**不会**被自动重写 ——
//!   一个卡组可能被多个卡池引用, 静默改写会同时改变多个卡池的行为.

use std::{cmp::Reverse, collections::HashMap, fs, path::PathBuf, sync::Arc};
use anyhow::{Context, Result};
use crate::{app::validation, domain::{deck::{Deck, EventGroup, Membership, TaggedDeck}, ids::GlobalId, localized_string::LocalizedString, origin::Origin, tag::{EventTag, Tag, Tagged}}, infrastructure::registry::{BannerRegistry, CardRegistry, DeckRegistry, LogicRegistry}, utils::{data_paths::DataPaths, path::atomic_write_json}};


/// 卡组管理器.
pub struct DeckManager {
    /// `DeckRegistry` 引用.
    deck_registry: Arc<DeckRegistry>,
    
    /// `CardRegistry` 引用, 用于卡组校验.
    card_registry: Arc<CardRegistry>,       // 其他注册器用于卡组修改/删除时的数据校验
    
    /// `BannerRegistry` 引用, 用于删除/更新前的引用检查.
    banner_registry: Arc<BannerRegistry>,
    
    /// `LogicRegistry` 引用, 用于卡池覆盖性校验.
    logic_registry: Arc<LogicRegistry>,
    
    /// 数据目录布局.
    paths: DataPaths,
}

impl DeckManager {
    /// 创建卡组管理器.
    pub fn new(
        deck_registry: Arc<DeckRegistry>,
        card_registry: Arc<CardRegistry>,
        banner_registry: Arc<BannerRegistry>,
        logic_registry: Arc<LogicRegistry>,
        paths: DataPaths,
    ) -> Self {
        Self {
            deck_registry,
            card_registry,
            banner_registry,
            logic_registry,
            paths,
        }
    }

    /// 用户可写的卡组目录 (`data/local/decks`).
    pub fn writable_dir(&self) -> PathBuf {
        self.paths.objects(&Origin::Local, "decks")
    }

    /// 获取所有可见卡组.
    pub fn list_all(&self) -> Vec<Arc<TaggedDeck>> {
        let mut decks = self.deck_registry.all_visible();
        decks.sort_by_key(|d| Reverse(d.global_id));    // 反转排序, 使最新创建的对象在前
        decks
    }

    /// 通过标签获取筛选后的可见卡组.
    /// 
    /// **该方法目前未被正式使用**.
    pub fn list_by_tags(&self, tags: &[Tag]) -> Vec<Arc<TaggedDeck>> {
        let ids = self.deck_registry.tag_index.query_all(tags);
        ids.into_iter()
            .filter(|id| self.deck_registry.is_visible(*id))
            .filter_map(|id| self.deck_registry.get_visible(id))
            .collect()
    }

    /// 创建一个新卡组并写入配置文件.
    pub fn create_deck(&self,
        name: LocalizedString,
        members: Membership,
        event_groups: HashMap<EventTag, EventGroup>,
        tags: Vec<Tag>
    ) -> Result<TaggedDeck> {
        if name.is_empty() {
            anyhow::bail!("卡组名称至少需要一种非空语言");
        }

        let mut deck = Deck::new(name);
        deck.members = members;
        deck.event_groups = event_groups;
        let deck = Tagged::with_tags(deck, tags.into_iter().collect());

        let path = self.build_file_path(deck.global_id);
        atomic_write_json(&path, &deck, "Deck")?;

        let id = deck.global_id;
        self.deck_registry.insert(deck.clone());
        self.deck_registry.insert_path(id, path);

        tracing::info!(deck_id = %id, "新建 Deck");

        Ok(deck)
    }

    /// 更新指定 Id 的卡组.
    /// 
    /// # 行为
    /// - 若目标卡组 `origin = local`, 就地更新 (更新前校验不会破坏引用它的卡池).
    /// - 若目标卡组来自只读来源, 转为**分叉**: 返回新建的派生卡组, 原对象不变.
    ///   此时**不会**改写任何卡池的 `deck_id`, 需要 UI 提示用户是否切换.
    pub fn update_deck(&self,
        id: GlobalId,
        name: Option<LocalizedString>,
        members: Option<Membership>,
        event_groups: Option<HashMap<EventTag, EventGroup>>,
        tags: Option<Vec<Tag>>,
    ) -> Result<TaggedDeck> {
        let old = self.deck_registry.get_including_shadowed(id)
            .ok_or_else(|| anyhow::anyhow!("Deck {} 不存在", id))?;

        if old.origin.is_read_only() {
            return self.fork_deck_from(old.as_ref(), |deck| {
                Self::apply_deck_updates(deck, name, members, event_groups, tags);
            });
        }

        // 构建新卡组
        let mut new_deck = old.as_ref().clone();
        Self::apply_deck_updates(&mut new_deck, name, members, event_groups, tags);

        if new_deck.name.is_empty() {
            anyhow::bail!("卡组名称至少需要一种非空语言");
        }

        // 校验卡组更新后的数据完整性
        for banner_id in self.banner_registry.find_banners_by_deck(id) {
            let banner = self.banner_registry.get_including_shadowed(banner_id)
                .ok_or_else(|| anyhow::anyhow!("Banner {} 不存在", banner_id))?;
            
            if let Err(e) = validation::check_banner_coverage(
                &banner,
                &new_deck,
                &self.card_registry,
                &self.logic_registry
            ) {
                tracing::warn!(
                    deck_id = %id.0,
                    banner_id = %banner_id.0,
                    error = %e,
                    "Deck 更新失败, 因为更新后将导致卡池校验失败"
                );
                anyhow::bail!("Deck {} 更新失败, 因为更新后将导致 Banner {} 校验失败: {}", id.0, banner_id.0, e)
            }
        }

        // 校验通过, 继续

        let new_path = self.build_file_path(new_deck.global_id);
        atomic_write_json(&new_path, &new_deck, "Deck")?;

        self.deck_registry.remove_with_path(id);
        self.deck_registry.insert(new_deck.clone());
        self.deck_registry.insert_path(id, new_path.clone());   // 自动覆盖原有路径

        // 数据格式 v2 下, 更新卡组不会更改卡组文件路径, 因此不存在需要删除的旧文件
        
        tracing::info!(deck_id = %id, "修改 Deck");

        Ok(new_deck)
    }

    /// 应用一组可选更新到卡组上.
    pub fn apply_deck_updates(
        deck: &mut TaggedDeck,
        name: Option<LocalizedString>,
        members: Option<Membership>,
        event_groups: Option<HashMap<EventTag, EventGroup>>,
        tags: Option<Vec<Tag>>
    ) {
        if let Some(name) = name { deck.name = name };
        if let Some(members) = members { deck.members = members };
        if let Some(event_groups) = event_groups { deck.event_groups = event_groups};
        if let Some(tags) = tags { deck.set_tags(tags.into_iter()); };
    }

    /// 由一个只读来源的卡组派生出一个新的本地卡组.
    /// 
    /// # 与 Card 分叉的差异
    /// 卡池对卡组的引用 (`Banner.deck_id`) **不自动重写**:
    /// 一个卡组通常被多个卡池引用, 自动改写会悄无声息地改变多个卡池的行为.
    /// UI 应提示用户是否切换到派生版本.
    pub fn fork_deck_from<F>(&self, source: &TaggedDeck, mutable: F) -> Result<TaggedDeck>
    where
        F: FnOnce(&mut TaggedDeck)
    {
        let existing = self.deck_registry.derived_of(source.global_id);
        if !existing.is_empty() {
            anyhow::bail!(
                "Deck {} 已存在派生对象 {:?}, 请编辑派生对象, 或先删除派生对象再重新派生",
                source.global_id,
                existing.iter().map(|id| id.to_string()).collect::<Vec<_>>()
            )
        }

        let mut derived = source.clone();
        derived.global_id = GlobalId::new();
        derived.origin = Origin::Local;
        derived.forked_from = Some(source.global_id);
        derived.shadowed = true;
        mutable(&mut derived);

        if derived.name.is_empty() {
            anyhow::bail!("卡组名称至少需要一种非空语言")
        }

        let source_id = source.global_id;
        let derived_id = derived.global_id;

        let path = self.build_file_path(derived_id);
        atomic_write_json(&path, &derived, "派生 Deck")?;

        self.deck_registry.insert(derived.clone());
        self.deck_registry.insert_path(derived_id, path);

        tracing::info!(source = %source_id, derived = %derived_id, "从只读来源分叉 Deck");

        Ok(derived)
    }

    /// 删除指定 Id 的卡组.
    /// 
    /// 若卡组被卡池引用, 拒绝删除.
    /// 若删除的是派生对象, 其"原对象"立即重新可见 (恢复官方版本).
    pub fn delete_deck(&self, id: GlobalId) -> Result<()> {
        let derived = self.deck_registry.derived_of(id);
        if !derived.is_empty() {
            anyhow::bail!(
                "Deck {} 存在派生对象 {:?}, 请先删除派生对象再删除原卡组",
                id, derived.iter().map(|d| d.to_string()).collect::<Vec<_>>()
            );
        }

        let deck = self.deck_registry.get_including_shadowed(id)
            .ok_or_else(|| anyhow::anyhow!("Deck {} 不存在", id))?;
        if deck.origin.is_read_only() {
            anyhow::bail!(
                "Deck {} 来自只读来源 `{}`, 不能删除; 如需修改请编辑派生对象",
                id.0, deck.origin
            );
        }

        let affected_banners = self.banner_registry.find_banners_by_deck(id);
        if !affected_banners.is_empty() {
            tracing::warn!(
                deck_id = %id.0,
                affected_banners = ?affected_banners.iter().map(|id| id.0).collect::<Vec<_>>(),
                "Deck 被 Banner 引用, 无法删除"
            );
            anyhow::bail!(
                "Deck {} 被下列 Banner {:?} 引用, 无法删除",
                id,
                affected_banners.iter().map(|id| id.to_string()).collect::<Vec<_>>()
            );
        }

        if let Some(path) = self.deck_registry.get_path(id) {
            if path.exists() {
                fs::remove_file(&path)
                    .with_context(|| format!("删除 Deck 文件失败 - file: {:?}", path))?;
            }
        }

        self.deck_registry.remove_with_path(id);
        
        tracing::info!(deck_id = %id.0, "删除 Deck");

        Ok(())
    }

    /// 构建卡组配置文件的存储路径
    /// 
    /// 格式: `data/local/decks/{global_id}.json`.
    pub fn build_file_path(&self, id: GlobalId) -> PathBuf {
        self.writable_dir().join(format!("{}.json", id))
    }
}