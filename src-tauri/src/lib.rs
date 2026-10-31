//! # Wishes 应用入口
//! 
//! `Tauri` 应用程序主入口.

pub mod app;
pub mod domain;
pub mod infrastructure;
pub mod interface;
pub mod utils;
use tauri::{Manager, State};
use anyhow::{Context, Result};
use tauri_plugin_opener::OpenerExt;
use crate::{
    app::state::AppState, domain::{
        ids::GlobalId,
        tag::Tag
    }, interface::{
        banner_info::{
            BannerInfo,
            BannerSummary
        },
        card_response::{
            CardCreateRequest,
            CardSummary,
            CardUpdateRequest
        },
        catalog_stats::CatalogStats,
        deck_response::{
            CreateDeckRequest,
            DeckSummary,
            UpdateDeckRequest
        },
        wish_response::WishResponse
    }, utils::path::{
        get_or_create_data_dir, get_or_create_db_dir, get_or_create_log_dir
    }
};

/// 记录 command 错误并转为前端字符串.
/// 
/// 所有 Tauri command 的错误出口都应经过此函数, 保证 release 构建下
/// 也能在日志中留下痕迹.
fn log_command_err(command: &'static str, e: anyhow::Error) -> String {
    tracing::error!(
        command = %command,
        error = %e,
        cause_chain = ?e.chain().map(|c| c.to_string()).collect::<Vec<_>>(),
        "Tauri command 失败"
    );
    e.to_string()
}

/// 执行单次抽卡
/// 
/// # 参数
/// - `banner_id`: 目标卡池的 `global_id` (Uuid 字符串).
/// - `state`: 当前应用状态.
/// 
/// # 返回值
/// 返回 `WishResponse` 包含抽到的卡片信息, 若失败则返回错误信息.
#[tauri::command]
fn wish(banner_id: GlobalId, state: State<AppState>) -> Result<WishResponse, String> {
    state.wish(banner_id)
        .map(WishResponse::new)
        .map_err(|e| log_command_err("wish", e))
}

/// 执行十连抽卡 (后端批量, 保证顺序与状态一致性)
/// 
/// # 参数
/// - `banner_id`: 目标卡池的 `global_id` (Uuid 字符串).
/// - `state`: 当前应用状态.
/// 
/// # 返回值
/// 按抽取顺序返回 10 个 `WishResponse`; 任一次抽取失败则整体返回错误.
#[tauri::command]
fn wish_ten(banner_id: GlobalId, state: State<AppState>) -> Result<Vec<WishResponse>, String> {
    let mut results = Vec::with_capacity(10);
    for _ in 0..10 {
        let result = state.wish(banner_id)
            .map_err(|e| log_command_err("wish_ten", e))?;
        results.push(WishResponse::new(result));
    }
    Ok(results)
}

/// 获取所有卡池的摘要列表.
/// 
/// 返回 `Vec<BannerSummary>` 供前端展示.
#[tauri::command]
fn get_banners(state: State<AppState>) -> Result<Vec<BannerSummary>, String> {
    Ok(state.banner_service.get_banner_summaries())
}

/// 获取指定卡池的详细信息.
/// 
/// # 参数
/// - `banner_id`: 卡池 `global_id` (Uuid 字符串).
/// 
/// 返回 `BannerInfo` 包含卡池名称、标签、关联卡组/逻辑名称及总抽数.
#[tauri::command]
fn get_banner_info(banner_id: GlobalId, state: State<AppState>) -> Result<BannerInfo, String> {
    state.banner_service.get_banner_info(banner_id)
        .map_err(|e| log_command_err("get_banner_info", e))
}

/// 获取当前图鉴的统计信息.
#[tauri::command]
fn get_catalog_stats(state: State<AppState>) -> Result<CatalogStats, String> {
    Ok(state.get_catalog_stats())
}

/// 获取当前使用的数据目录路径.
#[tauri::command]
fn get_data_dir_path(state: State<AppState>) -> Result<String, String> {
    Ok(state.data_dir.to_string_lossy().into_owned())
}

#[tauri::command]
/// 在系统文件管理器中打开数据目录.
fn open_data_dir(state: State<AppState>, handle: tauri::AppHandle) -> Result<(), String> {
    handle.opener()
        .open_path(state.data_dir.to_string_lossy().as_ref(), None::<&str>)
        .map_err(|e| {
            tracing::error!(error = %e, data_dir = %state.data_dir.display(), "打开数据目录失败");
            e.to_string()
        })
}

/// 获取应用日志目录.
/// 与获取数据目录的方式不同, log_dir 不存在多方引用的情况, 直接获取即可.
#[tauri::command]
fn get_log_dir_path(handle: tauri::AppHandle) -> Result<String, String> {
    get_or_create_log_dir(&handle)
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| log_command_err("get_log_dir_path", e))
}

/// 在系统文件管理器中打开日志目录.
#[tauri::command]
fn open_log_dir(handle: tauri::AppHandle) -> Result<(), String> {
    let log_dir = get_or_create_log_dir(&handle)
        .map_err(|e| log_command_err("open_log_dir", e))?;
    handle.opener()
        .open_path(log_dir.to_string_lossy().as_ref(), None::<&str>)
        .map_err(|e| {
            tracing::error!(error = %e, log_dir = %log_dir.display(), "打开日志目录失败");
            e.to_string()
        })
}

/// 获取所有卡片信息.
#[tauri::command]
fn list_cards(state: State<AppState>) -> Result<Vec<CardSummary>, String> {
    let cards = state.card_manager.list_all();
    Ok(cards.iter().map(|card| CardSummary::from_with_registry(card, &state.card_registry)).collect())
}

/// 获取根据标签筛选后的卡片的信息.
/// 目前前端实际未使用.
#[tauri::command]
fn list_cards_by_tags(tags: Vec<Tag>, state: State<AppState>) -> Result<Vec<CardSummary>, String> {
    let cards = state.card_manager.list_by_tags(&tags);
    Ok(cards.iter().map(|card| CardSummary::from_with_registry(card, &state.card_registry)).collect())
}

/// 创建新的卡片.
#[tauri::command]
fn create_card(req: CardCreateRequest, state: State<AppState>) -> Result<CardSummary, String> {
    let card = state.card_manager.create_card(req.content, req.tags, req.title)
        .map_err(|e| log_command_err("create_card", e))?;
    Ok(CardSummary::from_with_registry(&card, &state.card_registry))
}

/// 更新卡片信息.
#[tauri::command]
fn update_card(req: CardUpdateRequest, state: State<AppState>) -> Result<CardSummary, String> {
    let card = state.card_manager.update_card(
        req.global_id, 
        req.content,
        req.tags,
        req.title,
        req.shadow,
    ).map_err(|e| log_command_err("update_card", e))?;
    Ok(CardSummary::from_with_registry(&card, &state.card_registry))
}

/// 删除卡片.
#[tauri::command]
fn delete_card(global_id: GlobalId, state: State<AppState>) -> Result<(), String> {
    state.card_manager.delete_card(global_id).map_err(|e| log_command_err("delete_card", e))?;
    Ok(())
}

/// 手动设置派生卡片是否遮蔽.
#[tauri::command]
fn set_card_shadow(global_id: GlobalId, shadowed: bool, state: State<AppState>) -> Result<CardSummary, String> {
    let card = state.card_manager.set_shadow(global_id, shadowed)
        .map_err(|e| log_command_err("set_card_shadow", e))?;
    Ok(CardSummary::from_with_registry(&card, &state.card_registry))
}

/// 获取显式引用指定卡片的卡组列表.
/// 
/// 用于删除卡片前在前端提示可能受影响的卡组.
#[tauri::command]
fn get_card_referencing_decks(global_id: GlobalId, state: State<AppState>) -> Result<Vec<DeckSummary>, String> {
    let decks = state.card_manager.referencing_decks(global_id);
    Ok(decks.iter().map(|deck| DeckSummary::from_with_registry(deck.as_ref(), &state.deck_registry)).collect())
}

/// 获取所有卡组信息.
#[tauri::command]
fn list_decks(state: State<AppState>) -> Result<Vec<DeckSummary>, String> {
    let decks = state.deck_manager.list_all();
    Ok(decks.iter().map(|d| DeckSummary::from_with_registry(d, &state.deck_registry)).collect())
}

/// 获取根据标签筛选后的卡片的信息.
#[tauri::command]
fn list_decks_by_tags(tags: Vec<Tag>, state: State<AppState>) -> Result<Vec<DeckSummary>, String> {
    let decks = state.deck_manager.list_by_tags(&tags);
    Ok(decks.iter().map(|d| DeckSummary::from_with_registry(d, &state.deck_registry)).collect())
}

/// 创建新卡组.
#[tauri::command]
fn create_deck(req: CreateDeckRequest, state: State<AppState>) -> Result<DeckSummary, String> {
    let deck = state.deck_manager.create_deck(req.name, req.members, req.event_groups, req.tags)
        .map_err(|e| log_command_err("create_deck", e))?;
    Ok(DeckSummary::from_with_registry(&deck, &state.deck_registry))
}

/// 更新卡组.
/// 
/// 若目标卡组来自只读来源 (官方 / 拓展包), 该操作会分叉出一个本地副本,
/// 引用它的卡池不会被自动切换.
#[tauri::command]
fn update_deck(req: UpdateDeckRequest, state: State<AppState>) -> Result<DeckSummary, String> {
    let deck = state.deck_manager.update_deck(
        req.global_id,
        req.name,
        req.members,
        req.event_groups,
        req.tags
    ).map_err(|e| log_command_err("update_deck", e))?;
    Ok(DeckSummary::from_with_registry(&deck, &state.deck_registry))
}

/// 删除卡组.
#[tauri::command]
fn delete_deck(global_id: GlobalId, state: State<AppState>) -> Result<(), String> {
    state.deck_manager.delete_deck(global_id).map_err(|e| log_command_err("delete_deck", e))?;
    Ok(())
}

/// `Tauri` 应用启动入口.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle();

            let log_dir = get_or_create_log_dir(handle)
                .with_context(|| "获取日志目录失败")?;
            let log_guard = crate::utils::logging::init(&log_dir)
                .with_context(|| "初始化日志系统失败")?;

            app.manage(LogGuardState(log_guard));

            tracing::debug!(log_dir = %log_dir.display(), "日志系统已初始化");

            let data_dir = get_or_create_data_dir(handle)
                .with_context(|| "获取数据目录失败")?;
            let db_dir = get_or_create_db_dir(handle)
                .with_context(|| "获取数据库目录失败")?;
            
            let app_state = AppState::load(&data_dir, &db_dir).inspect_err(|e| {
                tracing::error!("Wishes 启动失败");
                for cause in e.chain() {
                    tracing::error!("- {}", cause);
                }
            })?;
            app.manage(app_state);
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            wish,
            wish_ten,
            get_banners,
            get_banner_info,
            get_catalog_stats,
            get_data_dir_path,
            open_data_dir,
            get_log_dir_path,
            open_log_dir,
            list_cards,
            list_cards_by_tags,
            create_card,
            update_card,
            delete_card,
            set_card_shadow,
            get_card_referencing_decks,
            list_decks,
            list_decks_by_tags,
            create_deck,
            update_deck,
            delete_deck,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Wishes");
}

/// 日志守卫的 Tauri 状态包装.
struct LogGuardState(#[allow(dead_code)] crate::utils::logging::LogGuard);
