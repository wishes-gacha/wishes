//! # 数据格式版本
//! 
//! 数据目录根部的 `version.json` 记录当前数据使用的格式版本:
//! 
//! ```json
//! { "format_version": 2 }
//! ```
//! 
//! 版本号为**整数**. 未来再次变更数据格式时, 该版本号**自增**.
//! 加载器在启动时读取并校验该文件, 版本不符时直接失败, 而不是尝试猜测或迁移.
//! 
//! # 为什么不做自动迁移
//! 
//! 当前 `Wishes` 仍处于测试阶段, 用户数量极少, 维护 N 个历史格式的迁移链
//! 成本远大于收益. 需要迁移本地的开发数据时, 使用 `scripts/` 下的 Python 脚本.

use std::path::Path;
use serde::{Deserialize, Serialize};

/// 当前实现支持的数据格式版本.
pub const DATA_FORMAT_VERSION: u32 = 2;

/// `version.json` 的文件名.
pub const VERSION_FILE_NAME: &str = "version.json";

/// 数据格式版本清单.
/// 
/// 对应 `data/version.json` 的内容.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataVersion {
    /// 数据格式版本号.
    pub format_version: u32,
}

impl DataVersion {
    /// 当前实现对应的版本.
    pub const fn current() -> Self {
        Self { format_version: DATA_FORMAT_VERSION }
    }

    /// 从数据目录根部读取 `version.json`.
    /// 
    /// # 错误
    /// - 文件不存在或无法读取.
    /// - 文件内容不是合法的 JSON, 或缺少 `format_version` 字段.
    pub fn load(data_dir: &Path) -> Result<Self, DataVersionError> {
        let path = data_dir.join(VERSION_FILE_NAME);

        let raw = std::fs::read_to_string(&path).map_err(|e| DataVersionError::Read {
            path: path.clone(),
            detail: e.to_string(),
        })?;

        serde_json::from_str(&raw).map_err(|e| DataVersionError::Parse {
            path,
            detail: e.to_string(),
        })
    }

    /// 校验版本是否与当前实现兼容.
    /// 
    /// # 错误
    /// 版本号与本实现不一致时返回 `DataVersionError::Mismatch`.
    pub fn ensure_supported(&self) -> Result<(), DataVersionError> {
        if self.format_version == DATA_FORMAT_VERSION {
            Ok(())
        } else {
            Err(DataVersionError::Mismatch {
                found: self.format_version,
                expected: DATA_FORMAT_VERSION,
            })
        }
    }

    /// 读取并校验数据目录的格式版本.
    pub fn load_and_verify(data_dir: &Path) -> Result<Self, DataVersionError> {
        let version = Self::load(data_dir)?;
        version.ensure_supported()?;
        Ok(version)
    }

    /// 将当前版本写入数据目录根部.
    /// 
    /// 仅由数据迁移脚本的等价实现 (或测试) 使用, 运行时不会写该文件.
    pub fn save(&self, data_dir: &Path) -> Result<(), DataVersionError> {
        let path = data_dir.join(VERSION_FILE_NAME);
        let raw = serde_json::to_string_pretty(self)
            .map_err(|e| DataVersionError::Parse { path: path.clone(), detail: e.to_string() })?;
        std::fs::write(&path, raw).map_err(|e| DataVersionError::Read { path, detail: e.to_string() })
    }
}

impl Default for DataVersion {
    fn default() -> Self {
        Self::current()
    }
}

/// 数据格式版本相关错误.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DataVersionError {
    /// 无法读取 `version.json`.
    #[error("无法读取数据格式版本文件 - file: {path}: {detail}")]
    Read {
        /// 文件路径.
        path: std::path::PathBuf,
        /// 底层错误描述.
        detail: String,
    },

    /// `version.json` 内容非法.
    #[error("解析数据格式版本文件失败 - file: {path}: {detail}")]
    Parse {
        /// 文件路径.
        path: std::path::PathBuf,
        /// 底层错误描述.
        detail: String,
    },

    /// 版本号与当前实现不一致.
    #[error(
        "数据格式版本不兼容: 数据目录为 v{found}, 当前程序需要 v{expected}. \
         请使用 scripts/ 下的迁移脚本更新数据, 或改用匹配版本的程序"
    )]
    Mismatch {
        /// 数据目录中记录的版本.
        found: u32,
        /// 当前实现支持的版本.
        expected: u32,
    },
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_version_is_v2() {
        assert_eq!(DataVersion::current().format_version, 2);
        assert!(DataVersion::current().ensure_supported().is_ok());
    }

    #[test]
    fn mismatched_version_is_rejected() {
        let v1 = DataVersion { format_version: 1 };
        assert!(matches!(
            v1.ensure_supported(),
            Err(DataVersionError::Mismatch { found: 1, expected: 2 })
        ));
    }

    #[test]
    fn round_trips_through_disk() {
        let dir = std::env::temp_dir().join(format!("wishes-version-test-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&dir).unwrap();

        DataVersion::current().save(&dir).unwrap();
        let loaded = DataVersion::load_and_verify(&dir).unwrap();
        assert_eq!(loaded, DataVersion::current());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_version_file_is_an_error() {
        let dir = std::env::temp_dir().join(format!("wishes-version-missing-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&dir).unwrap();

        assert!(matches!(
            DataVersion::load(&dir),
            Err(DataVersionError::Read { .. })
        ));

        std::fs::remove_dir_all(&dir).ok();
    }
}
