//! # 抽卡流程的最终结果.

use std::sync::Arc;
use crate::domain::{card::TaggedCard, tag::EventTag};

/// 每一抽的最终结果.
/// 包含标签化后的卡片 (`TaggedCard`) 和该卡片在当前卡池中的活动标签 (`EventTag`) 标签列表.
pub struct WishResult {
    /// 抽中的标签化卡片.
    pub card: Arc<TaggedCard>,

    /// 该卡片在当前卡组中的活动分组.
    pub event_tags: Vec<EventTag>,
}
