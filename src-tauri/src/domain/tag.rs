//! # 标签 Tag 模块
//! 
//! 提供通用的 标签 `Tag` 和 活动标签 `EventTag` 类型, 以及一个能将任意数据
//! 与标签集合绑定的 标签化包装器 `Tagged<T>`.
//! 
//! 主要功能:
//! - 通过 `Tag` 表示键值对形式的标签.
//! - 通过 `EventTag` 表示仅含值的活动标签 (主要用于抽卡流程).
//! - 通过 `Tagged<T>` 为任意数据结构附加 `HashSet<Tag>`, 并提供常用的标签增删查操作.


use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};
use serde::{Serialize, Deserialize};


/// 标签, 由 `namespace` 和 `value` 组成.
/// 
/// 这种设计允许不同命名空间下拥有相同 `value` 的标签共存.
/// 两个 标签 相等当且仅当 `namespace` 和 `value` 均相等.
/// 
/// # 示例
/// 
/// ```rust
/// # use wishes_lib::domain::tag::Tag;
/// let tag = Tag::new("rarity", "5");      // 五星稀有度标签
/// assert_eq!(tag.namespace, "rarity");
/// assert_eq!(tag.value, "5");
/// ```
/// 
/// 也可以通过元组转换:
/// ```rust
/// # use wishes_lib::domain::tag::Tag;
/// let tag: Tag = ("game", "genshin").into();
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub namespace: String,
    pub value: String,
}

impl PartialEq for Tag {
    fn eq(&self, other: &Self) -> bool {
        self.namespace == other.namespace && self.value == other.value
    }
}
impl Eq for Tag {}
impl Hash for Tag {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.namespace.hash(state);
        self.value.hash(state);
    }
}

impl Tag {
    /// 创建一个新的 `Tag`.
    pub fn new<T: Into<String>>(namespace: T, value: T) -> Self {
        Tag {namespace: namespace.into(), value: value.into()}
    }

    /// 内置命名空间常量: 用于标识游戏名.
    pub const NAMESPACE_GAME: &'static str = "game";

    /// 内置命名空间常量: 用于标识类型 (如角色、武器等).
    pub const NAMESPACE_TYPE: &'static str = "type";

    /// 内置命名空间常量: 用于标识稀有度 (主要用于卡片).
    pub const NAMESPACE_RARITY: &'static str = "rarity";
}

/// 允许从 `(K, V)` 元组转为 `Tag`, 其中 `K` 和 `V` 均需实现 `Into<String>`.
impl<K, V> From<(K, V)> for Tag
where
    K: Into<String>,
    V: Into<String>,
{
    fn from(value: (K, V)) -> Self {
        Tag {
            namespace: value.0.into(),
            value: value.1.into(),
        }
    }
}


/// 活动标签, 用于在抽卡流程中动态标记卡片归属的活动分组.
/// 
/// 卡片的活动分组是临时的. 例如, 同一张卡片, 在卡池 A 中可能只是一张
/// 普通卡片 (`standard`), 但在卡池 B 中可能成为概率提升卡片 (`up`).
/// 因此, 动态的 `EventTag` 诞生了.
/// 
/// 与 `Tag` 不同, `EventTag` 独立于卡片本身, 由 抽卡逻辑 产生, 
/// 用于指示本次抽取的卡片应属于 `up`, `standard`, `fes` 等分组.
/// 卡组内部也基于 `EventTag` 对卡片进行临时的分组.
/// 
/// # 示例
/// 
/// ```rust
/// # use wishes_lib::domain::tag::EventTag;
/// let event_tag = EventTag::new("up");
/// assert_eq!(event_tag.0, "up");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventTag(pub String);

impl PartialEq for EventTag {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for EventTag {}
impl Hash for EventTag {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl EventTag {
    /// 从任意可转换为 `String` 的类型创建一个新的 `EventTag`.
    pub fn new(value: impl Into<String>) -> Self {
        EventTag(value.into())
    }

    /// 快速创建代表 "up(概率提升)" 活动标签
    pub fn up() -> Self {
        EventTag::new("up")
    }

    /// 快速创建代表 "fes(节日限定)" 活动标签
    pub fn fes() -> Self {
        EventTag::new("fes")
    }

    /// 快速创建代表 "appoint(指定)" 活动标签
    pub fn appoint() -> Self {
        EventTag::new("appoint")
    }

    /// 快速创建代表 "standard(常规)" 活动标签
    pub fn standard() -> Self {
        EventTag::new("standard")
    }
}

/// 允许从 `&str` 转换为 `EventTag`.
impl From<&str> for EventTag {
    fn from(value: &str) -> Self {
        EventTag::new(value)
    }
}


/// 标签化包装器, 为任意类型 `T` 附加 `HashSet<T>`.
/// 
/// # 序列化行为
/// `inner` 通过 `#[serde(flatten)]` 展平, `tags` 作为额外的独立字段.
/// 若标签集合为空, JSON 中不会出现 `tags` 字段.
/// 
/// # 示例
/// 
/// ```rust
/// # use std::collections::HashSet;
/// # use wishes_lib::domain::tag::{Tag, Tagged};
/// let data = vec![1, 2, 3];
/// let mut tagged = Tagged::new(data);
/// tagged.add_tag(Tag::new("type", "numbers"));
/// assert!(tagged.has_tag(&Tag::new("type", "numbers")));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tagged<T> {
    #[serde(flatten)]
    pub inner: T,
    #[serde(default, skip_serializing_if = "HashSet::is_empty")]
    pub tags: HashSet<Tag>
}

impl<T> Tagged<T> {
    /// 创建一个不带初始标签的包装器.
    pub fn new(inner: T) -> Self {
        Self {inner, tags: HashSet::new()}
    }

    /// 创建一个待初始标签的包装器.
    pub fn with_tags(inner: T, tags: HashSet<Tag>) -> Self {
        Self {inner, tags}
    }

    /// 添加标签.
    pub fn add_tag(&mut self, tag: Tag) {
        self.tags.insert(tag);
    }

    /// 根据命名空间获取标签.
    /// 若存在多个相同命名空间标签, 只返回第一个匹配的.
    pub fn get_tag(&self, namespace: &str) -> Option<Tag> {
        for tag in &self.tags {
            if tag.namespace == namespace {
                return Some(tag.clone());
            }
        }
        None
    }

    /// 移除指定标签.
    pub fn remove_tag(&mut self, tag: &Tag) {
        self.tags.remove(tag);
    }

    /// 检查是否包含指定标签.
    pub fn has_tag(&self, tag: &Tag) -> bool {
        self.tags.contains(tag)
    }

    /// 检查是否包含所有给定的标签.
    pub fn contains_all(&self, tags: &[Tag]) -> bool {
        // TODO: 改用迭代器作为参数
        tags.iter().all(|tag| self.tags.contains(tag))
    }

    /// 获取标签迭代器.
    pub fn iter_tags(&self) -> impl Iterator<Item = &Tag> {
        self.tags.iter()
    }

    /// 传入迭代器替换整个标签集合
    pub fn set_tags(&mut self, tags: impl Iterator<Item = Tag>) {
        self.tags = tags.collect()
    }

    /// 按命名空间字母序排序后的标签列表 (用于接口响应与展示, 保证顺序稳定).
    pub fn sorted_tags(&self) -> Vec<Tag> {
        let mut tags: Vec<Tag> = self.tags.iter().cloned().collect();
        tags.sort_by(|a, b| a.namespace.cmp(&b.namespace).then_with(|| a.value.cmp(&b.value)));
        tags
    }
}

impl<T> Deref for Tagged<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T> DerefMut for Tagged<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}