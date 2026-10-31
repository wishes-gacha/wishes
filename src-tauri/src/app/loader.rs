//! # 数据加载器
//! 
//! 负责从文件系统加载各类数据 (卡片、卡组、逻辑、卡池) 并构建注册器.
//! 
//! # 加载顺序与来源
//! 
//! 数据格式 v2 把数据按**来源物理分离**, 加载时以 `official` -> `local` -> `packs/<name>`
//! 的顺序扫描, 因此用户数据总是覆盖在官方数据之上:
//! 
//! ```txt
//! data/official/<cards|decks|logics|banners>/*.json
//! data/local/<cards|decks|logics|banners>/*.json
//! data/packs/<name>/<cards|decks|logics|banners>/*.json
//! ```
//! 
//! 加载时会进行:
//! - **版本校验**: 读取 `data/version.json`, 版本不符直接失败.
//! - **唯一性校验**: 跨来源不允许出现重复的 `global_id`.
//! - **引用完整性校验**: 卡池引用的卡组与逻辑必须存在.
//! - **标签覆盖性校验**: 硬编码逻辑可能输出的标签组合, 卡组必须都能给出卡片.

use std::{collections::HashSet, fs, path::{Path, PathBuf}};
use anyhow::{Context, Result};
use serde::de::DeserializeOwned;
use walkdir::WalkDir;
use crate::{
    domain::{
        banner::TaggedBanner, card::TaggedCard, deck::{EventGroupCondition, TaggedDeck}, ids::GlobalId, logic::definition::{
            LogicVariant,
            TaggedLogicDefinition
        }, origin::Origin, version::DataVersion
    }, infrastructure::registry::{
        BannerRegistry,
        CardRegistry,
        DeckRegistry,
        LogicRegistry
    }, utils::data_paths::DataPaths
};

/// 加载器.
/// 所有方法均为静态风格.
pub struct Loader;

impl Default for Loader {
    fn default() -> Self {
        Self::new()
    }
}

impl Loader {
    /// 创建一个新的加载器.
    pub fn new() -> Self {
        Self {}
    }

    /// 按数据格式 v2 的目录布局加载**全部**数据.
    /// 
    /// # 流程
    /// 1. 校验 `data/version.json`.
    /// 2. 依次加载 official / local / packs 的卡片、卡组、逻辑、卡池.
    /// 
    /// # 错误
    /// 版本不符、Id 重复、引用缺失、标签覆盖不足等都会返回错误.
    pub fn load_all(&self, paths: &DataPaths) -> Result<(CardRegistry, DeckRegistry, LogicRegistry, BannerRegistry)> {
        DataVersion::load_and_verify(paths.root())
            .with_context(|| format!("数据格式版本校验失败 - dir: {}", paths.root().display()))?;

        // 1. 卡片
        let card_registry = CardRegistry::new();
        let mut seen_cards = HashSet::new();
        self.for_each_source(paths, "cards", |origin, path| {
            let card: TaggedCard = Self::read_json(path, "Card")?;
            Self::bind_origin(&card.origin, origin, card.global_id, "Card", path)?;
            Self::check_unique(&mut seen_cards, card.global_id, "Card", path)?;

            let id = card.global_id;
            card_registry.insert(card);
            card_registry.insert_path(id, path.to_path_buf());
            Ok(())
        })?;

        // 2. 卡组
        let deck_registry = DeckRegistry::new();
        let mut seen_decks = HashSet::new();
        self.for_each_source(paths, "decks", |origin, path| {
            let deck: TaggedDeck = Self::read_json(path, "Deck")?;
            Self::bind_origin(&deck.origin, origin, deck.global_id, "Deck", path)?;
            Self::check_unique(&mut seen_decks, deck.global_id, "Deck", path)?;
            Self::check_unsupported_event_conditions(&deck, path)?;

            let id = deck.global_id;
            deck_registry.insert(deck);
            deck_registry.insert_path(id, path.to_path_buf());
            Ok(())
        })?;

        // 3. 逻辑定义
        let logic_registry = LogicRegistry::new();
        let mut seen_logics = HashSet::new();
        self.for_each_source(paths, "logics", |origin, path| {
            let logic_def: TaggedLogicDefinition = Self::read_json(path, "Logic")?;
            Self::bind_origin(&logic_def.origin, origin, logic_def.global_id, "Logic", path)?;
            Self::check_unique(&mut seen_logics, logic_def.global_id, "Logic", path)?;

            if let LogicVariant::Hardcoded { executor_name } = &logic_def.variant {
                if !logic_registry.contains_hardcoded_executor(executor_name) {
                    anyhow::bail!(
                        "Logic `{}` 引用了未注册的硬编码执行器 (Hardcoded Executor) {} - file: {}",
                        logic_def.global_id, executor_name, path.display()
                    )
                }
            }

            logic_registry.insert_definition(logic_def);
            
            Ok(())
        })?;

        // 4. 卡池
        let banner_registry = BannerRegistry::new();
        let mut seen_banners = HashSet::new();
        self.for_each_source(paths, "banners", |origin, path| {
            let banner: TaggedBanner = Self::read_json(path, "Banner")?;
            Self::bind_origin(&banner.origin, origin, banner.global_id, "Banner", path)?;
            Self::check_unique(&mut seen_banners, banner.global_id, "Banner", path)?;

            Self::check_banner_references(&banner, &deck_registry, &logic_registry, path)?;
            Self::check_banner_coverage(&banner, &card_registry, &deck_registry, &logic_registry, path)?;

            let id = banner.global_id;
            banner_registry.insert(banner);
            banner_registry.insert_path(id, path.to_path_buf());

            Ok(())
        })?;

        Ok((card_registry, deck_registry, logic_registry, banner_registry))
    }

    /// 遍历全部数据来源 (`official` / `local` / `packs/*`) 中某类对象的目录.
    /// 
    /// `object_dir` 取值为 `cards` / `decks` / `logics` / `banners`.
    /// 目录不存在时静默跳过 —— 允许用户只备份 `data/local/`.
    fn for_each_source<F>(&self, paths: &DataPaths, object_dir: &str, mut f: F) -> Result<()>
    where 
        F: FnMut(&Origin, &Path) -> Result<()>
    {
        // official 与 local 是固定来源
        for origin in [Origin::Official, Origin::Local] {
            let dir = paths.objects(&origin, object_dir);
            self.for_each_json(&dir, &origin, &mut f)?;
        }

        let packs_dir = paths.packs();
        if packs_dir.is_dir() {
            let mut names: Vec<String> = fs::read_dir(&packs_dir)
                .with_context(|| format!("无法读取拓展包目录 - dir: {}", packs_dir.display()))?
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
                .collect();
            names.sort();

            for name in names {
                let origin = Origin::Pack(name);
                let dir = paths.objects(&origin, object_dir);
                self.for_each_json(&dir, &origin, &mut f)?;
            }
        }

        Ok(())
    }

    /// 遍历单个目录下的全部 JSON 文件 (递归).
    fn for_each_json<F>(&self, dir: &Path, origin: &Origin, f: &mut F) -> Result<()>
    where 
        F: FnMut(&Origin, &Path) -> Result<()>
    {
        if !dir.is_dir() {
            return Ok(());
        }

        let mut entries: Vec<PathBuf> = WalkDir::new(dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
            .map(|e| e.path().to_path_buf())
            .collect();
        entries.sort();             // 保证加载顺序稳定

        for path in entries {
            f(origin, &path)?;
        }

        Ok(())
    }

    /// 读取并解析一个 JSON 文件.
    fn read_json<T>(path: &Path, kind: &str) -> Result<T>
    where 
        T: DeserializeOwned
    {
        let data = fs::read_to_string(path)
            .with_context(|| format!("无法读取 {} 定义文件 - file: {}", kind, path.display()))?;
        serde_json::from_str(&data)
            .with_context(|| format!("解析 {} 定义文件中的 JSON 数据失败 - file: {}", kind, path.display()))
    }

    /// 校验对象声明的 `origin` 与其所在目录对应的来源一致.
    /// 
    /// 数据文件里的 `origin` 是冗余但重要的语义字段, 必须与物理位置一致,
    /// 否则"官方数据只读"的保证就失效了.
    fn bind_origin(declared: &Origin, located: &Origin, id: GlobalId, kind: &str, path: &Path) -> Result<()> {
        if declared != located {
            anyhow::bail!(
                "{} `{}` 声明的 origin 为 `{}`, 但其位于 `{}` 来源的目录中 - file: {}",
                kind, id, declared, located, path.display()
            );
        }
        Ok(())
    }

    /// 校验 `global_id` 在全部来源中唯一.
    fn check_unique(seen: &mut HashSet<GlobalId>, id: GlobalId, kind: &str, path: &Path) -> Result<()> {
        if !seen.insert(id) {
            anyhow::bail!("重复声明的 {} global_id `{}` - file: {}", kind, id, path.display());
        }
        Ok(())
    }

    /// 卡组中暂时不允许出现尚未支持的 `IncludeGroups` / `ExcludeGroups`.
    fn check_unsupported_event_conditions(deck: &TaggedDeck, path: &Path) -> Result<()> {
        for (event_tag, group) in &deck.event_groups {
            for cond in &group.conditions {
                match cond {
                    // IncludeIds 执行时会与 Deck::members 取交集, 自动过滤不存在 id
                    // 故这里不需要检查
                    EventGroupCondition::IncludeGroups { .. } |
                    EventGroupCondition::ExcludeGroups { .. } => {
                        anyhow::bail!(
                            "Deck {} 的 EventGroup {:?} 使用了未支持的 IncludeGroups/ExcludeGroups - file: {}",
                            deck.global_id, event_tag, path.display()
                        )
                    }
                    _ => {},
                }
            }
        }
        Ok(())
    }

    /// 校验卡池引用的卡组与逻辑定义存在.
    fn check_banner_references(
        banner: &TaggedBanner,
        deck_registry: &DeckRegistry,
        logic_registry: &LogicRegistry,
        path: &Path,
    ) -> Result<()> {
        if !logic_registry.contains_definition(banner.logic_id) {
            anyhow::bail!(
                "Banner {} 引用了未定义的 Logic {} - file: {}",
                banner.global_id, banner.logic_id, path.display()
            );
        }

        if !deck_registry.contains_including_shadowed(banner.deck_id) {
            anyhow::bail!(
                "Banner {} 引用了未定义的 Deck {} - file: {}",
                banner.global_id, banner.deck_id, path.display()
            );
        }

        Ok(())
    }

    /// 标签覆盖性检验: 硬编码逻辑可能输出的每个 (普通标签, 活动标签) 组合,
    /// 卡池使用的卡组都必须能查到至少一张卡片.
    fn check_banner_coverage(
        banner: &TaggedBanner,
        card_registry: &CardRegistry,
        deck_registry: &DeckRegistry,
        logic_registry: &LogicRegistry,
        path: &Path,
    ) -> Result<()> {
        let Some(tagged_deck) = deck_registry.get_including_shadowed(banner.deck_id) else {
            return Ok(());      // 引用缺失由 check_banner_references 负责报告
        };

        let Some(tagged_logic_def) = logic_registry.get_definition_including_shadowed(banner.logic_id) else {
            return Ok(());
        };

        if let LogicVariant::Hardcoded { executor_name } = &tagged_logic_def.variant {
            let mut missing = Vec::new();
            let combos = logic_registry.hardcoded_possible_output_combinations(executor_name)
                .unwrap_or_default();

            for combo in combos {
                let candidates = tagged_deck.query_cards(card_registry, &combo.0, &combo.1);
                if candidates.is_empty() {
                    missing.push(combo);
                }
            }

            if !missing.is_empty() {
                anyhow::bail!(
                    "Banner {} 中的 Deck {} 缺少 Logic {} 可能输出的 Tag 组合 - file: {}\n{}",
                    banner.global_id, tagged_deck.global_id, tagged_logic_def.global_id, path.display(),
                    missing.iter()
                        .map(|p| format!("  - {:?}", p))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
            }
        }

        Ok(())
    }

    // /// 从指定目录及其子目录加载所有 JSON 文件作为卡片定义, 注册至 `CardRegistry` 并返回.
    // /// 
    // /// # 校验
    // /// - 每个卡片的 Id 必须唯一.
    // /// - 若发现重复 Id 则返回错误.
    // pub fn load_cards_from_dir(&self, dir_path: &Path) -> Result<CardRegistry> {
    //     let card_registry = CardRegistry::new();
    //     let mut max_id = 0;

    //     for entry in WalkDir::new(dir_path)
    //         .into_iter()
    //         .filter_map(|e| e.ok())
    //         .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
    //     {
    //         let path = entry.path();
    //         let data = fs::read_to_string(path)
    //             .with_context(|| format!("无法读取 Card 定义文件 - file: {}", path.display()))?;
    //         let tagged_card: TaggedCard = serde_json::from_str(&data)
    //             .with_context(|| format!("解析 Card 文件中的 JSON 数据失败 - file: {}", path.display()))?;

    //         let id = tagged_card.id;                    // id 唯一性检查
    //         if card_registry.contains(id) {
    //             anyhow::bail!("重复声明的 Card Id `{}` - file: {}", id.0, path.display())
    //         }

    //         if id.0 > max_id {                          // 获取最大 Id
    //             max_id = id.0;
    //         }

    //         let tags: Vec<_> = tagged_card.tags.iter().cloned().collect();
    //         card_registry.tag_index().insert(id, &tags);
    //         card_registry.insert(tagged_card);
    //         card_registry.insert_path(id, path.to_path_buf());
    //     }

    //     card_registry.reset_id(max_id);

    //     Ok(card_registry)
    // }

    // /// 从指定目录及其子目录加载所有 JSON 文件作为卡组定义, 注册至 `DeckRegistry` 并返回.
    // /// 
    // /// # 校验
    // /// - 卡组 Id 唯一.
    // /// - 不会检查引用的卡片是否存在, 因为 members 和 event_groups 动态计算时自动过滤不存在卡片
    // pub fn load_decks_from_dir(&self, dir_path: &Path) -> Result<DeckRegistry> {
    //     let deck_registry = DeckRegistry::new();
    //     let mut max_id = 0;

    //     for entry in WalkDir::new(dir_path)
    //         .into_iter()
    //         .filter_map(|e| e.ok())
    //         .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
    //     {
    //         let path = entry.path();
    //         let data = fs::read_to_string(path)
    //             .with_context(|| format!("无法读取 Deck 定义文件 - file: {}", path.display()))?;
    //         let tagged_deck: TaggedDeck = serde_json::from_str(&data)
    //             .with_context(|| format!("解析 Deck 文件中的 JSON 数据失败 - file: {}", path.display()))?;

    //         let id = tagged_deck.id;                // id 唯一性检查
    //         if deck_registry.contains(id) {
    //             anyhow::bail!("重复声明的 Deck Id `{}` - file: {}", id.0, path.display())
    //         }

    //         if id.0 > max_id {
    //             max_id = id.0;
    //         }

    //         // Deck.members 现在改为动态规则, 自动过滤不存在 Id
    //         // 故无需检查 CardId 是否存在
            
    //         for (event_tag, group) in &tagged_deck.event_groups {
    //             for cond in &group.conditions {
    //                 // 此处的检查在 IncludeGroups 和 ExcludeGroups 实现后是不需要的
    //                 match cond {
    //                     // IncludeIds 执行时会与 Deck::members 取交集, 自动过滤不存在 id
    //                     // 故这里不需要检查
    //                     EventGroupCondition::IncludeGroups { .. } |
    //                     EventGroupCondition::ExcludeGroups { .. } => {
    //                         anyhow::bail!(
    //                             "Deck {} 的 EventGroup {:?} 使用了未支持的 IncludeGroups/ExcludeGroups - file: {}",
    //                             tagged_deck.id.0, event_tag, path.display()
    //                         )
    //                     }
    //                     _ => {},
    //                 }
    //             }
    //         }

    //         let tags: Vec<_> = tagged_deck.tags.iter().cloned().collect();
    //         deck_registry.tag_index.insert(id, &tags);
    //         deck_registry.insert(tagged_deck);
    //         deck_registry.insert_path(id, path.to_path_buf());
            
    //     }

    //     deck_registry.reset_id(max_id);

    //     Ok(deck_registry)
    // }

    // /// 从指定目录及其子目录加载所有 JSON 文件作为逻辑定义, 并注册至传入的 `logic_registry`.
    // /// 
    // /// # 校验
    // /// - 逻辑 Id 唯一.
    // /// - 如果逻辑变体为 `LogicVariant::Hardcoded`, 则引用的执行器名称必须已注册.
    // /// 
    // /// # 注意
    // /// 此方法需要修改传入的 `logic_registry`.
    // pub fn load_logics_from_dir(&self, dir_path: &Path, logic_registry: &mut LogicRegistry) -> Result<()> {
    //     for entry in WalkDir::new(dir_path)
    //         .into_iter()
    //         .filter_map(|e| e.ok())
    //         .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
    //     {
    //         let path = entry.path();
    //         let data = fs::read_to_string(path)
    //             .with_context(|| format!("无法读取 Logic 定义文件 - file: {}", path.display()))?;
    //         let tagged_logic_def: TaggedLogicDefinition = serde_json::from_str(&data)
    //             .with_context(|| format!("解析 Logic 文件中的 JSON 数据失败 - file: {}", path.display()))?;

    //         let id = tagged_logic_def.id;        // id 唯一性检查
    //         if logic_registry.contains_definition(id) {
    //             anyhow::bail!("重复声明的 Logic Id `{}` - file: {}", id.0, path.display())
    //         }

    //         if let LogicVariant::Hardcoded { executor_name } = &tagged_logic_def.variant {
    //             if !logic_registry.contains_hardcoded_executor(executor_name) {
    //                 anyhow::bail!(
    //                     "Logic {} 引用了未注册的硬编码执行器 (Hardcoded Executor) {} - file: {}",
    //                     tagged_logic_def.id.0, executor_name, path.display()
    //                 )
    //             }
    //         }
            
    //         let tags: Vec<_> = tagged_logic_def.tags.iter().cloned().collect();
    //         logic_registry.tag_index.insert(id, &tags);
    //         logic_registry.insert_definition(tagged_logic_def);
    //         // 添加路径

    //     }

    //     Ok(())
    // }

    // /// 从指定目录及其子目录加载所有 JSON 文件作为卡池定义, 注册至 `BannerRegistry` 中并返回.
    // /// 
    // /// # 校验
    // /// - 卡组 Id 唯一.
    // /// - 引用的逻辑定义和卡组必须存在.
    // /// - 如果逻辑为 `LogicVariant::Hardcoded`, 将检查卡组是否覆盖了所有可能的输出标签组合.
    // pub fn load_banner_from_dir(
    //     &self,
    //     dir_path: &Path,
    //     card_registry: &CardRegistry,
    //     deck_registry: &DeckRegistry,
    //     logic_registry: &LogicRegistry
    // ) -> Result<BannerRegistry> {
    //     let banner_registry = BannerRegistry::new();
        
    //     for entry in WalkDir::new(dir_path)
    //         .into_iter()
    //         .filter_map(|e| e.ok())
    //         .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("json"))
    //     {
    //         let path = entry.path();
    //         let data = fs::read_to_string(path)
    //             .with_context(|| format!("无法读取 Banner 定义文件 - file: {}", path.display()))?;
    //         let tagged_banner: TaggedBanner = serde_json::from_str(&data)
    //             .with_context(|| format!("解析 Banner 文件中的 JSON 数据失败 - file: {}", path.display()))?;
            
    //         let id = tagged_banner.id;              // id 唯一性检查
    //         if banner_registry.contains(id) {
    //             anyhow::bail!("重复声明的 Banner Id `{}` - file: {}", id.0, path.display())
    //         }

    //         // 检查 Logic
    //         if !logic_registry.contains_definition(tagged_banner.logic_id) {
    //             anyhow::bail!(
    //                 "Banner {} 引用了未定义的 Logic {} - file: {}",
    //                 tagged_banner.id.0, tagged_banner.logic_id.0, path.display()
    //             );
    //         }

    //         // 检查 Deck
    //         if let Some(tagged_deck) = deck_registry.get(tagged_banner.deck_id) {
    //             let tagged_logic_def = logic_registry.get_definition(tagged_banner.logic_id).unwrap();
    //             if let LogicVariant::Hardcoded { executor_name } = &tagged_logic_def.variant {
    //                 let mut missing = Vec::new();
    //                 let combos = logic_registry.hardcoded_possible_output_combinations(executor_name)
    //                     .unwrap_or_default();
    //                 for combo in combos {
    //                     let candidates = tagged_deck.query_cards(card_registry, &combo.0, &combo.1);
    //                     if candidates.is_empty() {
    //                         missing.push(combo);
    //                     }
    //                 }
    //                 if !missing.is_empty() {
    //                     anyhow::bail!(
    //                         "Banner {} 中的 Deck {} 缺少 Logic {} 可能输出的 Tag 组合 - file: {}\n{}",
    //                         tagged_banner.id.0, tagged_banner.id.0, tagged_logic_def.id.0, path.display(),
    //                         missing.iter()
    //                             .map(|p| format!("  - {:?}", p))
    //                             .collect::<Vec<_>>()
    //                             .join("\n")
    //                     );
    //                 }
    //             }

    //             let tags: Vec<_> = tagged_banner.tags.iter().cloned().collect();
    //             banner_registry.tag_index.insert(id, &tags);
    //             banner_registry.insert(tagged_banner);
    //             // 添加路径

    //         } else {
    //             anyhow::bail!(
    //                 "Banner {} 引用了未定义的 Deck {} - file: {}",
    //                 tagged_banner.id.0, tagged_banner.deck_id.0, path.display()
    //             );
    //         }
    //     }

    //     Ok(banner_registry)
    // }
}