//! # 数据校验
//! 
//! 供管理器在修改数据前进行的一致性检查.

use anyhow::Result;

use crate::{domain::{banner::TaggedBanner, deck::TaggedDeck, logic::definition::LogicVariant}, infrastructure::registry::{CardRegistry, LogicRegistry}};


/// 校验卡组是否覆盖了卡池逻辑可能输出的全部标签组合.
/// 
/// 硬编码逻辑会输出若干 (普通标签, 活动标签) 组合, 卡组必须能为每个组合
/// 至少提供一张卡片, 否则抽卡时会出现"候选卡片为空"的失败.
pub fn check_banner_coverage(
    banner: &TaggedBanner,
    deck: &TaggedDeck,
    card_registry: &CardRegistry,
    logic_registry: &LogicRegistry,
) -> Result<()> {
    let logic_def = logic_registry
        .get_definition_including_shadowed(banner.logic_id)
        .ok_or_else(|| anyhow::anyhow!("Logic 定义 {} 不存在", banner.logic_id.0))?;

    if let LogicVariant::Hardcoded { executor_name } = &logic_def.variant {
        let combos = logic_registry
            .hardcoded_possible_output_combinations(executor_name)
            .unwrap_or_default();

        let mut missing = Vec::new();
        for (tags, event_tags) in combos {
            let candidates = deck.query_cards(card_registry, &tags, &event_tags);
            if candidates.is_empty() {
                missing.push((tags, event_tags));
            }
        }

        if !missing.is_empty() {
            anyhow::bail!(
                "Deck {} 未能覆盖 Logic {} 的所有输出组合, 缺失组合: {:?}",
                deck.global_id,
                logic_def.global_id,
                missing
            )
        }
    }

    Ok(())
}
