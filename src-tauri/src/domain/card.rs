//! # 卡片 Card 定义
//! 包含卡片的基本信息.

use serde::{Serialize, Deserialize};
use crate::domain::{ids::GlobalId, localized_string::LocalizedString, origin::Origin, tag::Tagged};


/// 卡片核心数据 (不包含标签 `Tag`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    /// 全局唯一标识 (创建时生成, 永不变).
    pub global_id: GlobalId,

    /// 对象来源.
    /// 
    /// 即使是默认值 `"local"` 也**始终序列化**.
    pub origin: Origin,

    /// 派生自哪个对象的 `global_id`.
    /// 
    /// 非派生对象 (官方或拓展包对象) **不应**出现该字段.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forked_from: Option<GlobalId>,

    /// 派生对象是否遮蔽其原对象.
    /// 
    /// - `true` (默认): 原对象在列表与抽卡查询中被跳过.
    /// - `false`: 原对象仍可见, 用户可以同时看到原对象与派生对象.
    /// 
    /// 对 `forked_from = None` 的对象该字段无意义.
    #[serde(default = "crate::domain::helpers::default_true", skip_serializing_if = "crate::domain::helpers::is_true")]
    pub shadowed: bool,

    /// 卡片主内容 (至少一种语言).
    pub content: LocalizedString,

    /// 称号 / 副标题, 可选字段, 仅部分卡片需要.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<LocalizedString>,

    /// 立绘等资源引用.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assets: Option<CardAssets>,
}

impl Card {
    /// 以默认来源 (`local`) 创建一个仅含单语言内容的卡片.
    pub fn new(content: LocalizedString) -> Self {
        Self {
            global_id: GlobalId::new(),
            origin: Origin::Local,
            forked_from: None,
            shadowed: true,
            content,
            title: None,
            assets: None,
        }
    }
    
    /// 按给定语言解析卡片显示内容, 缺失时回退到默认语言或任意可用语言.
    pub fn display_content(&self, locale: &str) -> Option<&str> {
        self.content.get_or_default_locale(locale)
    }
}

/// 卡片资产引用.
/// 
/// 所有字段都是**相对于所属来源 `assets/` 目录**的路径,
/// 如 `"cards/0192a3c4-....png"` 对应 `data/official/assets/cards/0192a3c4-....png`.
/// 
/// # 缺失资产
/// 前端渲染时若文件不存在则显示占位图, 不报错.
/// 加载器启动时**不检查**资产文件存在性, 以避免大量文件 I/O.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardAssets {
    /// 立绘 (主资产).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub portrait: Option<String>,

    /// 缩略图.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,

    /// 动画 (预留).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<String>,
}

impl CardAssets {
    /// 是否所有资产引用都为空.
    pub fn is_empty(&self) -> bool {
        self.portrait.is_none() && self.thumbnail.is_none() && self.animation.is_none()
    }
}

/// 带标签的卡片, 实际存储和传递的类型.
/// 即 `Tagged<Card>`.
pub type TaggedCard = Tagged<Card>;


#[cfg(test)]
mod tests {
    use crate::domain::tag::Tag;

use super::*;

    #[test]
    fn serializes_according_to_v2_layout() {
        let card = Card::new(LocalizedString::from_pairs([("zh-CN", "胡桃"), ("en", "Hu Tao")]));
        let mut card = Tagged::new(card);
        card.origin = Origin::Official;
        card.add_tag(Tag::new("game", "genshin"));

        let value: serde_json::Value = serde_json::to_value(&card).unwrap();

        assert!(value["global_id"].is_string());
        assert_eq!(value["origin"], "official");
        assert_eq!(value["content"]["zh-CN"], "胡桃");
        assert_eq!(value["tags"][0]["namespace"], "game");
        // 可选字段为空时不应出现
        assert!(value.get("forked_from").is_none());
        assert!(value.get("title").is_none());
        assert!(value.get("assets").is_none());
    }

    #[test]
    fn round_trips_and_carries_fork_lineage() {
        let mut card = Card::new(LocalizedString::single("zh-CN", "胡桃"));
        card.forked_from = Some(GlobalId::new());
        card.assets = Some(CardAssets {
            portrait: Some("cards/abc.png".into()),
            ..Default::default()
        });

        let json = serde_json::to_string(&card).unwrap();
        let parsed: Card = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.forked_from, card.forked_from);
        assert_eq!(parsed.assets.unwrap().portrait.as_deref(), Some("cards/abc.png"));
    }
}