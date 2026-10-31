//! # 数据目录路径处理
//! 
//! 负责确定应用的数据目录位置, 并在必要时从资源目录复制默认数据.
//! 开发环境下优先使用项目目录下的 `data` 文件夹, 生产环境下使用用户数据目录.

use std::{fs, path::{Path, PathBuf}};
use anyhow::{Context, Result};
use serde::Serialize;
use tauri::{AppHandle, Manager, path::BaseDirectory};

use crate::utils::data_paths::DataPaths;
use crate::domain::version::{DataVersion, VERSION_FILE_NAME};

/// 获取最终使用的数据目录, 若目标目录不存在则资源中复制默认内容到目标目录.
/// 
/// # 行为
/// - **开发环境**: 检查项目目录下的 `data` 文件夹是否存在且有效,
///   若存在则直接使用.
/// - **生产环境**: 使用用户数据目录中的 `data` 目录, 若不存在,
///   则从资源目录中的 `data` 目录复制默认数据.
/// 
/// # 返回值
/// 返回最终可用的数据目录路径, 若任何步骤失败则返回错误.
pub fn get_or_create_data_dir(handle: &AppHandle) -> Result<PathBuf> {
    // 开发环境下使用项目数据
    #[cfg(debug_assertions)]
    {
        if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            let dev_dir = PathBuf::from(manifest_dir).join("../data");
            if dev_dir.exists() && check_data_dir(&dev_dir) {
                tracing::info!(data_dir = %dev_dir.display(), "使用开发环境数据目录");
                return Ok(dev_dir);
            }
        }
    }

    // 生成环境下使用用户数据目录数据
    let user_data_dir = handle
        .path()
        .app_data_dir()
        .with_context(|| "无法获取用户数据目录")?;
    let target = user_data_dir.join("data");

    // 仅在首次启动时复制, 已存在的用户数据不覆盖
    if !target.exists() {
        let source = handle
            .path()
            .resolve("data", BaseDirectory::Resource)
            .with_context(|| "无法解析资源路径 'data'")?;

        if source.exists() {
            tracing::info!(
                source = %source.display(),
                target = %target.display(),
                "首次启动：从资源目录复制默认数据"
            );

            // 复制前不再强校验 source 的完整性：
            //   - 打包工具可能遗漏空目录或 version.json
            //   - 缺失项在下方 ensure_version_file / AppState::ensure_layout 中兜底
            if !source.join("version.json").exists() {
                tracing::warn!(
                    source = %source.display(),
                    "资源 data 目录缺少 version.json (可能被遗漏), 将自动补写"
                );
            }

            copy_dir_all(&source, &target)?;
        } else {
            tracing::warn!(
                source = %source.display(),
                "资源目录中未找到 data, 数据目录将为空"
            );
            std::fs::create_dir_all(&target)
                .with_context(|| format!("创建数据目录失败 - dir: {}", target.display()))?;
        }
    }

    // 兜底: 确保 version.json 一定存在
    ensure_version_file(&target)?;

    tracing::info!(data_dir = %target.display(), "使用生成环境数据目录");

    Ok(target)
}

fn ensure_version_file(data_dir: &Path) -> Result<()> {
    let path = data_dir.join(VERSION_FILE_NAME);
    if path.exists() {
        return Ok(());
    }

    tracing::warn!(
        data_dir = %data_dir.display(),
        "数据目录缺少 version.json，自动写入当前数据格式版本"
    );
    DataVersion::current()
        .save(data_dir)
        .map_err(|e| anyhow::anyhow!("自动写入 version.json 失败: {}", e))?;
    Ok(())
}

/// 获取最终使用的数据库目录, 若不存在则自动创建
/// 
/// # 行为
/// - **开发环境**: 使用项目根目录下的 db 目录, 若不存在则创建.
/// - **生产环境**: 使用用户数据目录下的 db 目录, 若不存在则创建.
/// 
/// # 返回值
/// 返回最终可用的数据库目录路径, 若任何步骤失败则返回错误.
pub fn get_or_create_db_dir(handle: &AppHandle) -> Result<PathBuf> {
    #[cfg(debug_assertions)]
    {
        if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            let dev_dir = PathBuf::from(manifest_dir).join("../db");
            if !dev_dir.exists() {
                fs::create_dir_all(&dev_dir).with_context(|| "创建 db 目录失败")?;
            }
            tracing::info!(db_dir = %dev_dir.display(), "使用开发环境数据库目录");
            return Ok(dev_dir);
        }
    }
    
    let user_data_dir = handle
        .path()
        .app_data_dir()
        .with_context(|| "无法获取用户数据目录")?;

    let target = user_data_dir.join("db");

    if !target.exists() {
        fs::create_dir_all(&target).with_context(|| "创建 db 目录失败")?;
    }

    tracing::info!(db_dir = %target.display(), "使用生成环境数据库目录");

    Ok(target)
}

/// 获取应用日志目录, 若不存在则创建.
/// 
/// - dev 构建: 使用项目根目录下的 `logs/`
/// - release 构建: 使用 `app_data_dir()/logs`, 与 `data/`、`db/` 同级
pub fn get_or_create_log_dir(handle: &AppHandle) -> Result<PathBuf> {
    #[cfg(debug_assertions)]
    {
        if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            let dev_dir = PathBuf::from(manifest_dir).join("../logs");
            if !dev_dir.exists() {
                fs::create_dir_all(&dev_dir)
                    .with_context(|| "创建 dev logs 目录失败")?;
            }
            return Ok(dev_dir);
        }
    }

    // 生产环境下与 data / db 等同目录, 保持运行时产物目录一致
    let dir = handle
        .path()
        .app_data_dir()
        .with_context(|| "无法获取应用日志目录")?
        .join("logs");

    if !dir.exists() {
        fs::create_dir_all(&dir).with_context(|| "创建日志目录失败")?;
    }
    Ok(dir)
}

/// 辅助函数, 递归复制整个目录及其内容 (保留原目录结构).
/// 
/// 若目录不存在则自动创建, 保留源目录的目录结构和文件.
fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<()> {
    std::fs::create_dir_all(&dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.as_ref().join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// 检查数据目录是否具备数据格式 v2 的最低要求.
/// 
/// 复用 [`DataPaths::verify`]: 校验 `version.json` 存在且版本受支持,
/// 并确认 `official/` 与 `local/` 子目录存在.
pub fn check_data_dir(dir: &Path) -> bool {
    match DataPaths::new(dir).verify() {
        Ok(()) => true,
        Err(reason) => {
            tracing::warn!(
                data_dir = %dir.display(),
                reason = %reason,
                "数据目录校验未通过"
            );
            false
        }
    }
}


/// 原子写入 JSON 文件.
///
/// 实现: 先序列化到同目录下的临时文件, 再 `rename` 覆盖目标路径.
/// 目标路径已存在时会被原子替换.
pub fn atomic_write_json<T>(path: &Path, value: &T, kind: &str) -> Result<()>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("原子写入时创建目录失败 - dir: {:?}", parent))?;
    }

    let json_string = serde_json::to_string_pretty(value)
        .with_context(|| format!("{} 转为 JSON 文本时失败", kind))?;

    let temp_path = path.with_extension("tmp");
    fs::write(&temp_path, json_string)
        .with_context(|| format!("原子写入临时文件失败 - file: {:?}", temp_path))?;
    fs::rename(&temp_path, path)
        .with_context(|| format!("重命名临时文件失败 - from: {:?}, to: {:?}", temp_path, path))?;

    Ok(())
}