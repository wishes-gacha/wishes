//! # 卡池管理和抽卡总服务
//! 
//! 提供卡池管理、抽卡执行、运行时状态持久化和卡池相关信息查询等功能.
//! 
//! # 状态作用域 (数据格式 v2)
//! 
//! 运行时状态不再以卡池自身为键, 而是以 `scope_key` 为键:
//! 
//! - `"banner:<uuid>"`: 该卡池独立维护状态 (默认).
//! - `"group:<name>"`: 同组的多个卡池**共享**同一份状态.
//! 
//! 故 `states` 缓存按 `scope_key` 分片, 而非按 `BannerId`.

use std::{collections::HashSet, sync::Arc};
use anyhow::{Context, Result};
use dashmap::DashMap;
use rand::{SeedableRng, rngs::ChaCha12Rng, seq::IndexedRandom};
use crate::{
    app::logic_engine::LogicEngine, domain::{
        banner::BannerRuntimeState, ids::{GlobalId, UserId}, wish_result::WishResult,
    }, infrastructure::{
        registry::{BannerRegistry, CardRegistry, DeckRegistry, LogicRegistry},
        repository::state_repo::StateRepository,
    }, interface::banner_info::{BannerInfo, BannerSummary},
};


/// 卡池管理和抽卡总服务.
/// 
/// 持有注册器、逻辑引擎、状态数据库, 以及**内存中的卡池运行时状态缓存**.
/// 
/// 状态缓存 (`states`) 是抽卡流程的工作区:
/// - 启动时由 `restore_states` 从数据库恢复.
/// - 抽卡时原地更新并写回数据库.
/// - 查询卡池信息时读取 (如 `total_counter`).
pub struct BannerService {
    /// 卡片注册器引用.
    card_registry: Arc<CardRegistry>,

    /// 卡组注册器引用.
    deck_registry: Arc<DeckRegistry>,

    /// 逻辑注册器引用.
    logic_registry: Arc<LogicRegistry>,

    /// 卡池注册器引用.
    banner_registry: Arc<BannerRegistry>,

    /// 逻辑引擎引用.
    logic_engine: Arc<LogicEngine>,

    /// 逻辑状态数据库引用.
    state_repository: Arc<StateRepository>,

    /// 每个**状态作用域**一份独立运行时状态, 与 `Banner` 分离.
    /// 
    /// 使用 `DashMap` 按 `scope_key` 分片, 并发抽不同卡池互不阻塞.
    /// 未来多用户时改为 `DashMap<(UserId, String), BannerRuntimeState>`.
    states: DashMap<String, BannerRuntimeState>,
}

impl BannerService {
    pub fn new(
        card_registry: Arc<CardRegistry>,
        deck_registry: Arc<DeckRegistry>,
        logic_registry: Arc<LogicRegistry>,
        banner_registry: Arc<BannerRegistry>,
        logic_engine: Arc<LogicEngine>,
        state_repository: Arc<StateRepository>,
    ) -> Self {
        Self {
            card_registry,
            deck_registry,
            logic_registry,
            banner_registry,
            logic_engine,
            state_repository,
            states: DashMap::new(),
        }
    }

    /// 执行单次抽卡.
    /// 
    /// # 流程
    /// 1. 从 `BannerRegistry` 读取卡池 (不可变数据, 无需加锁).
    /// 2. 计算卡池的状态作用域, 从内存状态缓存中锁定对应的 `BannerRuntimeState`.
    /// 3. 读取 `total_counter`, 构造确定性随机种子.
    /// 4. 调用逻辑引擎执行抽卡逻辑, 就地更新 `state.logic_state`.
    /// 5. 根据逻辑结果查询候选卡片, 等概率随机选取一张.
    /// 6. 将 `state.total_counter + 1` 写回数据库.
    /// 7. 返回抽卡结果.
    /// 
    /// # 错误
    /// - 卡池不存在, 或其状态未初始化.
    /// - 逻辑执行失败.
    /// - 候选卡片列表为空 (说明卡组未覆盖逻辑的所有可能输出).
    /// - 随机选取或状态持久化失败.
    pub fn wish(&self, banner_id: GlobalId) -> Result<WishResult> {
        // 1. Banner 是不可变的, 无需加锁
        let banner = self.banner_registry.get_including_shadowed(banner_id)
            .ok_or_else(|| anyhow::anyhow!("未找到 Banner `{}`", banner_id))?;
        
        // 2. 计算状态作用域并锁定运行时状态
        let scope_key = banner.state_scope_key();

        let mut state_guard = self.states.get_mut(&scope_key)
            .ok_or_else(|| anyhow::anyhow!(
                "Banner {} (scope `{}`) 的运行时状态未初始化 (可能未调用 restore_states)",
                banner_id, scope_key
            ))?;
        let state = state_guard.value_mut();

        // 3. 构造随机种子
        let total_counter = state.total_counter;
        let seed = self.build_seed("0", banner_id, total_counter); // profile 固定为 0, TODO: 后期添加多用户
        let mut rng = ChaCha12Rng::seed_from_u64(seed);

        let mut new_state = state.clone();                  // 新建状态副本, 所有修改先在副本上进行
        new_state.total_counter = total_counter + 1;

        // 4. 执行逻辑, 更新状态副本的 logic_state
        let result = self.logic_engine.execute(
            banner.logic_id,
            &mut new_state.logic_state,
            &mut rng,
        )?;

        // 5. 查询候选卡片
        let tagged_deck = self.deck_registry.get_including_shadowed(banner.deck_id)
            .ok_or_else(|| anyhow::anyhow!("未找到 Deck `{}`", banner.deck_id))?;

        let candidates = tagged_deck.query_cards(
            &self.card_registry,
            &result.tags,
            &result.event_tags,
        );

        if candidates.is_empty() {
            let member_count = tagged_deck.members.resolve(&self.card_registry).len();
            let msg = format!(
                "Deck {} 没有 Logic {} 匹配 Tag {:?} 和 EventTag {:?} 的 Card, 当前 Deck 成员数量: {}",
                tagged_deck.global_id,
                banner.logic_id,
                result.tags,
                result.event_tags,
                member_count,
            );
            tracing::error!(
                banner_id = %banner.global_id,
                deck_id = %tagged_deck.global_id,
                logic_id = %banner.logic_id,
                tags = ?result.tags,
                event_tags = ?result.event_tags,
                member_count = %member_count,
                msg
            );
            anyhow::bail!(msg);
        }

        let picked_id = candidates.choose(&mut rng)
            .ok_or_else(|| anyhow::anyhow!("随机抽取 Card GlobalId 失败!"))?;

        // 6. 更新计数器并持久化
        // 持久化的是**修改后**的新状态
        self.state_repository.save(UserId(0), &scope_key, &new_state)
            .with_context(|| format!("Banner {} 存储运行时状态时失败", banner_id.0))?;
        *state = new_state;     // 入库成功后, 再用副本替换原本的状态, 保证原子提交

        // 7. 返回结果
        let tagged_card = self.card_registry.get_including_shadowed(*picked_id)
            .ok_or_else(|| anyhow::anyhow!("未找到 Card: Id `{}`", picked_id.0))?;

        tracing::info!(
            banner_id = %banner_id.0,
            card_id = %picked_id.0,
            total_counter = %state.total_counter,
            seed = %seed,
            tags = ?result.tags,
            event_tags = ?result.event_tags,
            "正常抽卡"
        );

        Ok(WishResult {
            card: tagged_card,
            event_tags: result.event_tags.to_vec(),
        })
    }

    /// 从指定用户 (profile)、卡池和抽卡次数构建确定性随机种子.
    /// 
    /// 使用 FNV-1a 算法基于 本地用户 Id, 卡池 Id, 总抽数 构建确定的随机种子.
    /// 当前 `profile` 固定为 "0" (单用户模式).
    pub fn build_seed(&self, profile: &str, banner_id: GlobalId, total_counter: u64) -> u64 {
        const FNV_OFFSET: u64 = 0xcbf29ce484222325;
        const FNV_PRIME:  u64 = 0x100000001b3;

        let mut hash = FNV_OFFSET;
        let mut mix = |bytes: &[u8]| {
            for &b in bytes {
                hash ^= b as u64;
                hash = hash.wrapping_mul(FNV_PRIME);
            }
        };

        mix(profile.as_bytes());
        mix(banner_id.as_uuid().as_bytes());
        mix(&total_counter.to_le_bytes());
        hash
    }

    /// 从数据库恢复所有卡池作用域的运行时状态到内存缓存 `states`.
    /// 
    /// 遍历 `BannerRegistry` 中的全部卡池, 按其 `scope_key` 去重后逐一加载.
    /// 若某作用域在数据库中尚无记录 (首次启动), 则填充默认值
    /// (计数器 0, 逻辑状态为空).
    /// 
    /// 应在 `BannerService` 构造完成后、首次抽卡前调用一次.
    pub fn restore_states(&self) -> Result<()> {
        let mut seen_scopes = HashSet::new();

        for banner in self.banner_registry.all_including_shadowed() {
            let scope_key = banner.state_scope_key();
            if !seen_scopes.insert(scope_key.clone()) {
                continue;       // 同作用域卡池共享状态, 只需加载一次
            }

            let state = self.state_repository
                .load(UserId(0), &scope_key)?
                .unwrap_or_default();
            self.states.insert(scope_key, state);
        }

        Ok(())
    }

    /// 获取所有**可见**卡池的摘要信息列表 `Vec<BannerSummary>`,
    /// 按 `global_id` 排序 (相当于按创建时间排序).
    /// 
    /// 被派生对象遮蔽的卡池不会出现在列表中.
    pub fn get_banner_summaries(&self) -> Vec<BannerSummary> {
        let mut summaries: Vec<_> = self.banner_registry
            .all_visible_ids()
            .iter()
            .filter_map(|id| {
                let banner = self.banner_registry.get_including_shadowed(*id)?;
                Some(BannerSummary::from(banner.as_ref()))
            })
            .collect();

        // Uuid v7 本身时间有序, 这里按 global_id 排序等价于按创建时间排序
        summaries.sort_by_key(|s| s.global_id);
        summaries
    }

    /// 读取某个作用域的当前总抽数.
    fn total_counter_of(&self, scope_key: &str) -> u64 {
        self.states
            .get(scope_key)
            .map(|s| s.total_counter)
            .unwrap_or(0)
    }

    /// 获取指定卡池的详细信息 `BannerInfo`.
    pub fn get_banner_info(&self, banner_id: GlobalId) -> Result<BannerInfo> {
        let banner = self.banner_registry.get_including_shadowed(banner_id)
            .ok_or_else(|| anyhow::anyhow!("未找到 Banner `{}`", banner_id))?;

        // 从内存状态缓存读取 total_counter
        let total_counter = self.total_counter_of(&banner.state_scope_key());

        let deck = self.deck_registry.get_including_shadowed(banner.deck_id)
            .ok_or_else(|| anyhow::anyhow!("未找到 Deck `{}`", banner.deck_id))?;

        let logic_def = self.logic_registry.get_definition_including_shadowed(banner.logic_id)
            .ok_or_else(|| anyhow::anyhow!("未找到 Logic `{}`", banner.logic_id))?;

        Ok(BannerInfo {
            global_id: banner.global_id,
            name: banner.name.clone(),
            tags: banner.sorted_tags(),
            deck_id: deck.global_id,
            logic_id: logic_def.global_id,
            deck_name: deck.name.clone(),
            logic_name: logic_def.name.clone(),
            state_scope: banner.state_scope.clone(),
            total_counter,
        })
    }
}