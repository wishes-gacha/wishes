//! # 应用全局状态
//! 
//! 集成所有注册器、核心服务，并提供统一的初始化和操作入口.
//! 在应用启动时从文件系统加载所有数据, 并建立服务间的依赖关系.


use std::{path::{Path, PathBuf}, sync::Arc};
use anyhow::{Context, Ok, Result};
use crate::{
    app::{
        admin::{card_manager::CardManager, deck_manager::DeckManager}, banner_service::BannerService, logic_engine::LogicEngine
    }, domain::{
        ids::GlobalId,
        wish_result::WishResult
    }, infrastructure::{
        registry::{
            BannerRegistry,
            CardRegistry,
            DeckRegistry,
            LogicRegistry
        },
        repository::{
            overlay_repo::{OverlayRepository, RefKind},
            state_repo::StateRepository
        }
    }, interface::catalog_stats::CatalogStats, utils::data_paths::DataPaths
};
use super::loader::Loader;

/// 应用状态, 持有所有注册器和服务实例.
/// 
/// 作为顶层容器, 提供对外的接口方法.
pub struct AppState {
    /// 卡片注册器引用
    pub card_registry: Arc<CardRegistry>,

    /// 卡组注册器引用.
    pub deck_registry: Arc<DeckRegistry>,

    /// 逻辑注册器引用.
    pub logic_registry: Arc<LogicRegistry>,

    /// 卡池注册器引用.
    pub banner_registry: Arc<BannerRegistry>,

    /// 卡片管理器实例.
    pub card_manager: CardManager,

    /// 卡组管理器实例.
    pub deck_manager: DeckManager,

    /// 逻辑引擎引用.
    pub logic_engine: Arc<LogicEngine>,

    /// 卡池服务实例.
    pub banner_service: BannerService,

    /// 当前使用的数据目录布局.
    pub paths: DataPaths,

    /// 存储当前使用的数据目录路径.
    pub data_dir: PathBuf,
}

impl AppState {
    /// 从指定数据目录加载所有配置和状态, 构建完整的 `AppState`.
    /// 
    /// # 流程
    /// 1. 校验 `data/version.json`, 确认数据格式版本.
    /// 2. 使用 `Loader` 按 official / local / packs 来源顺序加载全部数据到注册器.
    /// 3. 将注册器包装为 `Arc` 引用.
    /// 4. 创建逻辑引擎和卡池服务.
    /// 5. 打开数据数据库并加载所有卡池作用域的持久化状态.
    /// 
    /// # 错误
    /// 任何加载或初始化步骤失败都将返回错误.
    pub fn load(data_dir: &Path, db_dir: &Path) -> Result<Self> {
        let loader = Loader::new();
        let paths = DataPaths::new(data_dir);

        paths.ensure_layout()
            .with_context(|| format!("创建闪数据子目录失败 - dir: {}", data_dir.display()))?;

        let (
            card_registry,
            deck_registry,
            logic_registry,
            banner_registry
        ) = loader.load_all(&paths)?;

        tracing::info!(
            card = card_registry.count(),           // 这些 count() 方法由 impl_fork_ops 宏提供
            deck = deck_registry.count(),
            logic = logic_registry.count(),
            banner = banner_registry.count(),
            "已完成数据的初始化加载"
        );

        // 转为 Arc 指针
        let card_registry = Arc::new(card_registry);
        let deck_registry = Arc::new(deck_registry);
        let logic_registry = Arc::new(logic_registry);
        let banner_registry = Arc::new(banner_registry);

        // 打开引用覆盖层数据库, 并按分叉/遮蔽状态重建卡片引用覆盖表.
        // 覆盖关系完全由数据文件中的 forked_from / shadowed 推导,
        // 重建可以自动清理陈旧记录.
        let overlay_repository = Arc::new(OverlayRepository::new(&db_dir.join("overrides.db"))?);
        let card_overrides = card_registry.ref_overrides_snapshot();
        overlay_repository
            .replace_all(RefKind::Card, &card_overrides)
            .with_context(|| "重建卡片引用覆盖表失败")?;
        tracing::info!(
            card_overrides = card_overrides.len(),
            "引用覆盖层已就绪 (官方数据文件不被改写)"
        );

        // 初始化管理器
        let card_manager = CardManager::new(
            card_registry.clone(),
            deck_registry.clone(),
            overlay_repository.clone(),
            paths.clone()
        );
        let deck_manager = DeckManager::new(
            deck_registry.clone(),
            card_registry.clone(),
            banner_registry.clone(),
            logic_registry.clone(),
            paths.clone()
        );
        
        // 初始化逻辑执行引擎
        let logic_engine = Arc::new(LogicEngine::new(logic_registry.clone()));

        // 连接逻辑状态数据库
        let state_repository = Arc::new(StateRepository::new(&db_dir.join("states.db"))?);

        let banner_service = BannerService::new(
            card_registry.clone(),
            deck_registry.clone(),
            logic_registry.clone(),
            banner_registry.clone(),
            logic_engine.clone(),
            state_repository.clone(),
        );

        banner_service.restore_states()?;

        Ok(Self {
            card_registry,
            deck_registry,
            logic_registry,
            banner_registry,
            card_manager,
            deck_manager,
            logic_engine,
            banner_service,
            paths,
            data_dir: data_dir.to_path_buf(),
        })
    }

    /// 执行一次抽卡, 委托给 `banner_service`.
    pub fn wish(&self, banner_id: GlobalId) -> Result<WishResult> {
        self.banner_service.wish(banner_id)
    }

    /// 获取图鉴统计信息.
    pub fn get_catalog_stats(&self) -> CatalogStats {
        CatalogStats {
            cards: self.card_registry.count_visible(),
            decks: self.deck_registry.count_visible(),
            logics: self.logic_registry.count_visible_definitions(),
        }
    }
}