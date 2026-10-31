//! # 卡片管理器
//! 
//! 管理卡片的创建、查询、更新、删除, 同时管理卡片文件.
//! 
//! # 数据格式 v2 的行为变化
//! 
//! - 新建卡片时生成 `GlobalId` (Uuid v7), 文件名为 `{global_id}.json`,
//!   直接落在**来源目录**的 `cards/` 下 (不再按标签值划分子目录).
//! - 编辑一个**只读来源** (官方 / 拓展包) 的卡片 = **分叉**: 新建一个派生对象,
//!   内容复制自原对象, `origin` 改为 `local`, `forked_from` 指向原对象.
//!   原对象从不被改写, 因此不存在冲突.
//! - 分叉/遮蔽**不写回任何卡组文件**: 卡组中对原卡片的显式引用由本地
//!   **引用覆盖层** (`OverlayRepository`, SQLite) 在解析时重定向到派生对象.
//!   官方数据与拓展包数据永远不会被改写.

use std::{cmp::Reverse, fs, path::PathBuf, sync::Arc};
use anyhow::{Context, Result};
use crate::{domain::{card::{Card, TaggedCard}, deck::TaggedDeck, ids::GlobalId, localized_string::LocalizedString, origin::Origin, tag::Tag}, infrastructure::{registry::{CardRegistry, DeckRegistry}, repository::overlay_repo::{OverlayRepository, RefKind}}, utils::{data_paths::DataPaths, path::atomic_write_json}};


/// 卡片管理器.
///
/// 管理卡片的创建、查询、更新、删除, 同时管理卡片文件.
pub struct CardManager {
    /// `CardRegistry` 引用.
    card_registry: Arc<CardRegistry>,

    /// `DeckRegistry` 引用, 用于引用检查与分叉时的引用重定向.
    deck_registry: Arc<DeckRegistry>,

    /// 引用覆盖层数据库引用, 用于持久化卡片的引用覆盖关系.
    overlay_repo: Arc<OverlayRepository>,

    /// 数据目录布局.
    paths: DataPaths,
}

impl CardManager {
    pub fn new(
        card_registry: Arc<CardRegistry>,
        deck_registry: Arc<DeckRegistry>,
        overlay_repo: Arc<OverlayRepository>,
        paths: DataPaths,
    ) -> Self {
        Self {
            card_registry,
            deck_registry,
            overlay_repo,
            paths,
        }
    }

    /// 用户可写的卡片目录 (`data/local/cards`).
    pub fn writable_dir(&self) -> PathBuf {
        self.paths.objects(&Origin::Local, "cards")
    }

    /// 获取所有**可见**卡片的 `Arc` 引用.
    pub fn list_all(&self) -> Vec<Arc<TaggedCard>> {
        let mut cards = self.card_registry.all_visible();
        cards.sort_by_key(|c| Reverse(c.global_id));    // 反转排序, 使最新创建的对象在前
        cards
    }

    /// 通过标签获取筛选后的**可见**卡片的 `Arc` 引用.
    /// 
    /// **该方法目前未被正式使用**.
    pub fn list_by_tags(&self, tags: &[Tag]) -> Vec<Arc<TaggedCard>> {
        let ids = self.card_registry.tag_index.query_all(tags);
        ids.into_iter()
            .filter(|id| self.card_registry.is_visible(*id))
            .filter_map(|id| self.card_registry.get_visible(id))
            .collect()
    }
    
    /// 创建一个新的卡片, 写入配置文件, 并返回为 `TaggedCard`.
    /// 
    /// 新卡片 `origin = local`, 文件写入 `data/local/cards/{global_id}.json`
    /// 
    /// # 失败
    /// 若写入文件失败, 则不会创建卡片.
    pub fn create_card(&self, content: LocalizedString, tags: Vec<Tag>, title: Option<LocalizedString>) -> Result<TaggedCard> {
        if content.is_empty() {
            anyhow::bail!("卡片内容至少需要一种非空语言");
        }

        let mut card = Card::new(content);
        card.title = title.filter(|t| !t.is_empty());
        let card = TaggedCard::with_tags(card, tags.into_iter().collect());

        // 写入文件
        let path = self.build_file_path(card.global_id);
        atomic_write_json(&path, &card, "Card")?;

        let id = card.global_id;
        self.card_registry.insert(card.clone());
        self.card_registry.insert_path(id, path);

        tracing::info!(
            card_id = %id,
            origin = %card.origin,
            "创建 Card"
        );

        Ok(card)
    }

    /// 更新指定 `global_id` 的卡片信息, 返回新的卡片为 `TaggedCard`.
    /// 
    /// # 行为
    /// - 若目标卡片 `origin = local`, 直接原地更新。
    /// - 若目标卡片来自只读来源 (官方 / 拓展包), 则**分叉**, 返回一个新建的
    ///   派生对象, 原对象保持不变并被遮蔽. 调用方可通过返回值的 `forked_from` 判断是否发生了分叉.
    /// 
    /// # 失败
    /// - 指定 `global_id` 的卡片不存在
    /// - 写入文件失败
    /// 
    /// 任何失败, 都不会改动原对象.
    pub fn update_card(&self,
        id: GlobalId,
        content: LocalizedString,
        tags: Vec<Tag>,
        title: Option<LocalizedString>,
        shadow: Option<bool>,
    ) -> Result<TaggedCard> {
        if content.is_empty() {
            anyhow::bail!("卡片内容至少需要一种非空语言");
        }
        
        // 获取旧卡片信息
        let old_card = self.card_registry.get_including_shadowed(id)
            .ok_or_else(|| anyhow::anyhow!("Card {} 不存在", id.0))?;
        
        // 只读来源 -> 分叉
        if old_card.origin.is_read_only() {
            return self.fork_card_from(old_card.as_ref(), shadow.unwrap_or(true), |card| {
                card.content = content;
                card.set_tags(tags.into_iter());
                card.title = title.filter(|t| !t.is_empty());
            });
        }

        // local 来源 -> 原地修改

        // 构建新卡片
        let mut new_card = old_card.as_ref().clone();
        new_card.content = content;
        new_card.set_tags(tags.into_iter());
        new_card.title = title.filter(|t| !t.is_empty());

        let new_path = self.build_file_path(new_card.global_id);

        // 尝试写入新文件
        atomic_write_json(&new_path, &new_card, "Card")?;

        // 更新内存
        self.card_registry.remove_with_path(id);
        self.card_registry.insert(new_card.clone());
        self.card_registry.insert_path(id, new_path.clone());

        // 数据格式 v2 下, 更新卡片不会更改卡片文件路径, 因此不存在需要删除的旧文件

        tracing::info!(card_id = %id, "修改 Card");

        Ok(new_card)
    }

    /// 由一个只读来源的卡片派生出一个新的本地卡片.
    ///
    /// # 流程 (对应数据分叉模型)
    /// 1. 复制原对象内容, 分配新的 `global_id`.
    /// 2. `origin` 改为 `local`, `forked_from` 指向原对象.
    /// 3. 写入 `data/local/cards/`.
    /// 4. 原对象保持不变, 并在列表/抽卡查询中被**遮蔽**.
    /// 5. 遮蔽时在引用覆盖层登记 `原卡片 -> 派生卡片`, 卡组文件保持不变.
    pub fn fork_card_from<F>(&self, source: &TaggedCard, shadow: bool, mutable: F) -> Result<TaggedCard>
    where
        F: FnOnce(&mut TaggedCard),
    {
        let source_id = source.global_id;

        let existing = self.card_registry.derived_of(source_id);
        if !existing.is_empty() {
            anyhow::bail!(
                "Card {} 已存在派生对象 {:?}, 请编辑派生对象, 或先删除派生对象再重新派生",
                source_id,
                existing.iter().map(|id| id.to_string()).collect::<Vec<_>>()
            )
        }
        
        let mut derived = source.clone();
        derived.global_id = GlobalId::new();
        derived.origin = Origin::Local;
        derived.forked_from = Some(source.global_id);
        derived.shadowed = shadow;
        mutable(&mut derived);

        let path = self.build_file_path(derived.global_id);
        atomic_write_json(&path, &derived, "派生 Card")?;

        let source_id = source.global_id;
        let derived_id = derived.global_id;

        self.card_registry.insert(derived.clone());
        self.card_registry.insert_path(derived_id, path);

        // 遮蔽时, 引用覆盖层把对原对象的引用解析到派生对象;
        // 不遮蔽时引用保持指向原对象, 派生对象作为备份存在.
        // 两种情况下都不改写任何卡组文件.
        self.sync_ref_overrides();
        if shadow {
            tracing::info!(
                source_id = %source_id,
                derived_id = %derived_id,
                "Card 分叉: 已登记引用覆盖 (不修改卡组文件)"
            );
        }

        tracing::info!(
            source_id = %source_id, derived_id = %derived_id, shadow,
            "从只读来源 (官方/拓展包) 分叉 Card"
        );

        Ok(derived)
    }

    /// 查询显式引用指定卡片的卡组列表.
    /// 
    /// 仅统计卡组成员规则中通过 `IncludeIds` 显式引用的卡片;
    /// 通过标签规则动态包含的卡片不计入.
    pub fn referencing_decks(&self, id: GlobalId) -> Vec<Arc<TaggedDeck>> {
        self.deck_registry.decks_referencing_card(id)
    }

    /// 删除指定 `global_id` 的卡片.
    /// 
    /// 若删除卡片文件失败, 则不会删除卡片并返回错误.
    /// 若卡片被卡组显式引用, 仅记录警告日志, 不阻止删除.
    /// 
    /// 若删除的是派生对象, 其"原对象"立即重新可见 (恢复官方版本).
    pub fn delete_card(&self, id: GlobalId) -> Result<()> {
        let derived = self.card_registry.derived_of(id);
        if !derived.is_empty() {
            anyhow::bail!(
                "Card {} 存在派生对象 {:?}, 需先删除派生对象后才能删除原卡片",
                id,
                derived.iter().map(|d| d.to_string()).collect::<Vec<_>>()
            );
        }

        let card = self.card_registry.get_including_shadowed(id)
            .ok_or_else(|| anyhow::anyhow!("Card {} 不存在", id))?;

        if card.origin.is_read_only() {
            anyhow::bail!(
                "Card {} 来自只读来源 `{}`, 不能删除; 如需修改请编辑派生对象",
                id.0, card.origin
            );
        }

        let source_id = card.forked_from;
        let is_fork = source_id.is_some();
        drop(card);     // 释放读锁

        // 被 Deck 显式引用时记录警告 (不阻止删除)
        let mut included_in = Vec::new();
        let mut excluded_in = Vec::new();

        for deck in self.referencing_decks(id) {
            let refs = deck.card_refs(id);
            if refs.included { included_in.push(deck.global_id.to_string()); }
            if refs.excluded { excluded_in.push(deck.global_id.to_string()); }
        }

        if is_fork {
            if !included_in.is_empty() || !excluded_in.is_empty() {
                tracing::info!(
                    card_id = %id,
                    source = %source_id.unwrap(),
                    included_in = ?included_in,
                    excluded_in = ?excluded_in,
                    "删除派生 Card: 显式引用将重写回原对象, 卡组内容不丢失"
                );
            }
        } else {
            if !included_in.is_empty() {
                tracing::warn!(
                    card_id = %id,
                    decks = ?included_in,
                    "Card 被 Deck 显式 Include, 删除后相关卡组将不再包含该卡片"
                )
            }
            if !excluded_in.is_empty() {
                tracing::warn!(
                    card_id = %id,
                    decks = ?excluded_in,
                    "Card 被 Deck 显式 Exclude, 删除后该 ExcludeIds 规则将指向不存在的卡片"
                )
            }
        }

        if let Some(path) = self.card_registry.get_path(id) {
            if path.exists() {
                fs::remove_file(&path)
                    .with_context(|| format!("删除 Card 文件失败 - file: {:?}", path))?;
            }
        }

        self.card_registry.remove_with_path(id);

        // 分叉关系的解除已在 CardRegistry 中处理, 此处无需考虑

        // 引用覆盖随派生对象移除由 CardRegistry 自动解除:
        // 指向原对象的引用重新解析到原对象, 卡组文件从未被改写.
        self.sync_ref_overrides();
        if let Some(source) = source_id {
            tracing::info!(
                derived = %id, source = %source,
                "删除派生 Card: 引用覆盖已移除, 对原对象的引用恢复生效"
            );
        }

        // TODO[2026-10-14]: loader.rs 加载时可遍历检查所有派生对象的 forked_from 是否存在并记录警告

        tracing::info!(card_id = %id.0, "删除 Card");
        Ok(())
    }

    /// 构建卡片配置文件的存储路径.
    /// 
    /// 路径格式: `data/local/cards/{global_id}.json`
    fn build_file_path(&self, id: GlobalId) -> PathBuf {
        self.writable_dir().join(format!("{}.json", id))
    }

    pub fn set_shadow(&self, id: GlobalId, shadowed: bool) -> Result<TaggedCard> {
        let card = self.card_registry.get_including_shadowed(id)
            .ok_or_else(|| anyhow::anyhow!("Card {} 不存在", id))?;

        let source = card.forked_from
            .ok_or_else(|| anyhow::anyhow!("Card {} 不是派生对象, 无法设置遮蔽", id))?;

        if card.shadowed == shadowed {
            return Ok(card.as_ref().clone());
        }

        let mut updated = card.as_ref().clone();
        updated.shadowed = shadowed;

        let path = self.card_registry.get_path(id)
            .ok_or_else(|| anyhow::anyhow!("Card {} 的文件路径未记录", id))?;
        atomic_write_json(&path, &updated, "Card")?;
        self.card_registry.insert(updated.clone());

        // 引用覆盖随遮蔽状态由 CardRegistry 自动维护; 不写回任何卡组文件
        self.sync_ref_overrides();

        tracing::info!(card_id = %id, shadowed, source = %source, "设置 Card 遮蔽状态 (引用覆盖已更新)");
        Ok(updated)
    }

    /// 把内存中的引用覆盖快照同步到本地覆盖层数据库.
    ///
    /// 数据库是覆盖关系的持久化副本, 启动时会按分叉/遮蔽状态重建;
    /// 因此这里的写入失败只记录警告, 不影响本次会话的正确性.
    fn sync_ref_overrides(&self) {
        let snapshot = self.card_registry.ref_overrides_snapshot();
        if let Err(e) = self.overlay_repo.replace_all(RefKind::Card, &snapshot) {
            tracing::warn!(
                error = %e,
                entries = snapshot.len(),
                "同步卡片引用覆盖到覆盖层数据库失败"
            );
        }
    }
}