//! # 数据目录布局 (数据格式 v2)
//! 
//! 集中定义数据目录结构, 避免各处硬编码路径字符串.
//! 
//! ```txt
//! data/
//! ├── version.json        # 数据格式版本
//! ├── official/           # 官方数据, 只读, 通过增量同步更新
//! │   ├── cards/  decks/  logics/  banners/
//! │   └── assets/{cards,decks,banners}/
//! ├── local/              # 用户数据 (从零创建 + 全部派生对象)
//! │   ├── cards/  decks/  logics/  banners/
//! │   └── assets/{cards,decks,banners}/
//! └── packs/              # 导入的拓展包
//!     └── <pack-name>/
//!         ├── pack.zip     # 原始 zip (备份, 不参与运行时)
//!         ├── cards/  decks/  logics/  banners/
//!         └── assets/{cards,decks,banners}/
//! ```
//! 
//! # 命名规则
//! 
//! - 对象文件: `{global_id}.json`
//! - 资产文件: `{global_id}.{ext}`, 同一对象多个资产用后缀区分 (如 `_thumb`)
//! - **不再按标签值划分子目录**, 消除了标签值路径穿越的风险

use std::path::{Path, PathBuf};

use crate::domain::origin::Origin;

/// 官方数据目录名.
pub const OFFICIAL_DIR: &str = "official";

/// 用户数据目录名.
pub const LOCAL_DIR: &str = "local";

/// 拓展包目录名.
pub const PACKS_DIR: &str = "packs";

/// 拓展包原始 zip 的固定文件名 (备份用, 不参与运行时).
pub const PACK_ZIP_NAME: &str = "pack.zip";

/// 资产目录名.
pub const ASSETS_DIR: &str = "assets";

/// 数据目录中四类核心对象对应的子目录名.
pub const OBJECT_DIRS: [&str; 4] = ["cards", "decks", "logics", "banners"];


/// 数据目录布局.
/// 
/// 仅持有根路径, 具体子路径按需拼装, 构造极其廉价.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataPaths {
    root: PathBuf,
}

impl DataPaths {
    /// 以给定根目录创建一个布局.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// 数据目录根路径.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 官方数据目录.
    pub fn official(&self) -> PathBuf {
        self.root.join(OFFICIAL_DIR)
    }

    /// 用户数据目录.
    pub fn local(&self) -> PathBuf {
        self.root.join(LOCAL_DIR)
    }

    /// 拓展包存放目录.
    pub fn packs(&self) -> PathBuf {
        self.root.join(PACKS_DIR)
    }

    /// 指定名称的拓展包目录.
    pub fn pack(&self, name: &str) -> PathBuf {
        self.packs().join(name)
    }

    /// 拓展包原始 zip 的路径.
    pub fn pack_zip(&self, name: &str) -> PathBuf {
        self.pack(name).join(PACK_ZIP_NAME)
    }

    /// 某个来源下, 某类对象 (`cards` / `decks` / `logics` / `banners`) 的目录.
    pub fn objects(&self, origin: &Origin, object_dir: &str) -> PathBuf {
        self.source_root(origin).join(object_dir)
    }

    /// 某个来源的根目录.
    pub fn source_root(&self, origin: &Origin) -> PathBuf {
        match origin {
            Origin::Official => self.official(),
            Origin::Local => self.local(),
            Origin::Pack(name) => self.pack(name),
        }
    }

    /// 某个来源下, 某类对象资产的目录 (如 `official/assets/cards`).
    pub fn assets(&self, origin: &Origin, asset_dir: &str) -> PathBuf {
        self.source_root(origin).join(ASSETS_DIR).join(asset_dir)
    }

    /// 把对象资产引用解析为磁盘上的绝对路径.
    /// 
    /// # 参数
    /// - `origin`: 资产所属来源 (`assets` 路径相对于该来源的 `assets/` 目录).
    /// - `relative`: 资产引用, 如 `"cards/0192a3c4-....png"`.
    /// 
    /// # 返回
    /// 资产文件的完整路径. 是否实际存在由调用方决定 ——
    /// 加载器**不检查**资产存在性, 以避免大量文件 I/O.
    pub fn resolve_asset(&self, origin: &Origin, relative: &str) -> PathBuf {
        self.source_root(origin).join(ASSETS_DIR).join(relative)
    }

    /// 校验数据目录是否具备 v2 布局的最低要求.
    /// 
    /// 检查项:
    /// - `version.json` 存在且格式版本受支持.
    /// - `official/` 与 `local/` 目录存在.
    pub fn verify(&self) -> Result<(), String> {
        if !self.root.is_dir() {
            return Err(format!("数据目录不存在: {}", self.root.display()));
        }

        crate::domain::version::DataVersion::load_and_verify(&self.root)
            .map_err(|e| e.to_string())?;

        for dir in [self.official(), self.local()] {
            if !dir.is_dir() {
                return Err(format!("数据目录缺少必要子目录: {}", dir.display()));
            }
        }

        Ok(())
    }

    /// 创建所有必需的数据子目录 (已存在时不做任何事).
    pub fn ensure_layout(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.root.join(OFFICIAL_DIR))?;
        std::fs::create_dir_all(self.root.join(LOCAL_DIR))?;
        std::fs::create_dir_all(self.root.join(PACKS_DIR))?;

        for source in [self.official(), self.local()] {
            for object_dir in OBJECT_DIRS {
                std::fs::create_dir_all(source.join(object_dir))?;
            }
            for asset_dir in OBJECT_DIRS {
                std::fs::create_dir_all(source.join(ASSETS_DIR).join(asset_dir))?;
            }
        }

        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_expected_layout() {
        let paths = DataPaths::new("/data");

        assert_eq!(paths.official(), PathBuf::from("/data/official"));
        assert_eq!(paths.local(), PathBuf::from("/data/local"));
        assert_eq!(paths.packs(), PathBuf::from("/data/packs"));
        assert_eq!(paths.pack("genshin"), PathBuf::from("/data/packs/genshin"));
        assert_eq!(
            paths.pack_zip("genshin"),
            PathBuf::from("/data/packs/genshin/pack.zip")
        );
    }

    #[test]
    fn object_dirs_are_per_origin() {
        let paths = DataPaths::new("/data");

        assert_eq!(
            paths.objects(&Origin::Official, "cards"),
            PathBuf::from("/data/official/cards")
        );
        assert_eq!(
            paths.objects(&Origin::Local, "decks"),
            PathBuf::from("/data/local/decks")
        );
        assert_eq!(
            paths.objects(&Origin::Pack("p1".into()), "banners"),
            PathBuf::from("/data/packs/p1/banners")
        );
    }

    #[test]
    fn resolves_asset_paths_relative_to_origin() {
        let paths = DataPaths::new("/data");

        assert_eq!(
            paths.resolve_asset(&Origin::Official, "cards/abc.png"),
            PathBuf::from("/data/official/assets/cards/abc.png")
        );
        assert_eq!(
            paths.resolve_asset(&Origin::Pack("p1".into()), "decks/abc.png"),
            PathBuf::from("/data/packs/p1/assets/decks/abc.png")
        );
    }

    #[test]
    fn ensure_layout_creates_all_required_directories() {
        let root = std::env::temp_dir().join(format!("wishes-paths-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&root).unwrap();
        crate::domain::version::DataVersion::current().save(&root).unwrap();

        let paths = DataPaths::new(&root);
        paths.ensure_layout().unwrap();

        // 全新检出时 local/ 与 packs/ 可能缺失, ensure_layout 必须补齐, verify 才通过
        assert!(paths.verify().is_ok(), "补齐布局后校验应通过: {:?}", paths.verify());

        for object_dir in OBJECT_DIRS {
            assert!(paths.official().join(object_dir).is_dir());
            assert!(paths.local().join(object_dir).is_dir());
            assert!(paths.official().join(ASSETS_DIR).join(object_dir).is_dir());
            assert!(paths.local().join(ASSETS_DIR).join(object_dir).is_dir());
        }
        assert!(paths.packs().is_dir());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn verify_rejects_directory_without_version_file() {
        let root = std::env::temp_dir().join(format!("wishes-paths-nover-{}", uuid::Uuid::now_v7()));
        std::fs::create_dir_all(&root).unwrap();

        assert!(DataPaths::new(&root).verify().is_err());

        std::fs::remove_dir_all(&root).ok();
    }
}