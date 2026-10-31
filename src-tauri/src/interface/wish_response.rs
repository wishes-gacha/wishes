//! # 抽卡响应
//! 
//! 定义抽卡后返回给前端的完整结果, 包含卡片内容、标签及活动标签.

use serde::Serialize;
use crate::domain::{ids::GlobalId, localized_string::LocalizedString, tag::{EventTag, Tag}, wish_result::WishResult};

/// 每一抽最终返回前端的响应数据, 包含该抽得到的卡片的完整信息.
#[derive(Debug, Clone, Serialize)]
pub struct WishResponse {
    /// 卡片 `global_id`.
    pub global_id: GlobalId,

    /// 卡片内容 (多语言), 由前端按当前界面语言回退解析.
    pub content: LocalizedString,
    
    /// 卡片称号 / 副标题 (多语言).
    pub title: Option<LocalizedString>,

    /// 卡片拥有的普通标签列表 (按命名空间排序).
    pub tags: Vec<Tag>,

    /// 卡片在当前卡池中附带的活动标签列表.
    pub event_tags: Vec<EventTag>
}

impl WishResponse {
    /// 消耗 `domain` 的 `WishResult` 并转换为前端响应.
    /// 
    /// 会提取卡片内容、克隆标签, 并对普通标签按命名空间顺序进行字母序排序.
    pub fn new(res: WishResult) -> Self {
        Self {
            global_id: res.card.global_id,
            content: res.card.content.clone(),
            title: res.card.title.clone(),
            tags: res.card.sorted_tags(),
            event_tags: res.event_tags,
        }
    }
}
