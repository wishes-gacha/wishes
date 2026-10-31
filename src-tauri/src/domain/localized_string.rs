//! # 多语言字符串 `LocalizedString`
//! 
//! 数据格式 v2 起, 所有面向用户的文本 (卡片内容、卡组名、卡池名等) 都以
//! **语言标签 -> 文本** 的映射存储, 以支持官方数据与拓展包携带多语言内容.
//! 
//! # 约定
//! - 键使用 BCP 47 语言标签, 如 `"zh-CN"`、`"en"`、`"ja"`.
//! - **不存储空字符串**: 某种语言未填写时, 该键不存在于映射中.
//! - 保存时至少一种语言非空 (前端与后端共同校验).

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// 默认语言标签.
/// 
/// 当请求的语言与默认语言都缺失时, 会回退到映射中的任意一种语言.
pub const DEFAULT_LOCALE: &str = "zh-CN";

/// 多语言字符串.
/// 
/// 序列化时**透明**为一个 `{ "<语言标签>": "<文本>" }` 对象,
/// 即 `LocalizedString` 与 `HashMap<String, String>` 的 JSON 形式完全一致.
/// 
/// # 示例
/// 
/// ```
/// # use wishes_lib::domain::localized_string::LocalizedString;
/// # use std::collections::HashMap;
/// let mut s = LocalizedString::new();
/// s.insert("zh-CN", "胡桃");
/// s.insert("en", "Hu Tao");
/// 
/// assert_eq!(s.get_with_fallback("ja", "zh-CN"), Some("胡桃"));
/// assert_eq!(s.get_with_fallback("en", "zh-CN"), Some("Hu Tao"));
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LocalizedString(pub HashMap<String, String>);

impl LocalizedString {
    /// 创建一个空的多语言字符串.
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// 由 `(语言标签, 文本)` 迭代器创建.
    /// 
    /// 文本为空的条目会被自动丢弃, 以维持"不存储空字符串"的约定.
    pub fn from_pairs<I, K, V>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let mut map = HashMap::new();
        for (locale, text) in pairs {
            let (locale, text) = (locale.into(), text.into());
            if !text.is_empty() {
                map.insert(locale, text);
            }
        }
        Self(map)
    }

    /// 由单个语言创建.
    /// 
    /// 若文本为空则返回空的多语言字符串.
    pub fn single(locale: impl Into<String>, text: impl Into<String>) -> Self {
        Self::from_pairs([(locale, text)])
    }

    /// 插入一个翻译.
    /// 
    /// 若文本为空, 则移除该语言的条目 (而非存入空字符串).
    pub fn insert(&mut self, locale: impl Into<String>, text: impl Into<String>) {
        let (locale, text) = (locale.into(), text.into());
        if text.is_empty() {
            self.0.remove(&locale);
        } else {
            self.0.insert(locale, text);
        }
    }

    /// 按 当前语言 -> 默认语言 -> 任意可用语言 的顺序查找文本.
    /// 
    /// # 参数
    /// - `locale`: 当前语言标签.
    /// - `default_locale`: 默认语言标签, 在前者缺失时回退.
    /// 
    /// # 返回
    /// 找到则返回文本引用, 映射为空时返回 `None`.
    pub fn get_with_fallback(&self, locale: &str, default_locale: &str) -> Option<&str> {
        self.0.get(locale)
            .or_else(|| self.0.get(default_locale))
            .or_else(|| self.0.values().next())
            .map(|s| s.as_str())
    }

    /// 使用默认语言 `zh-CN` 进行回退查找.
    pub fn get_or_default_locale(&self, locale: &str) -> Option<&str> {
        self.get_with_fallback(locale, DEFAULT_LOCALE)
    }

    /// 获取指定语言的文本, 不做回退.
    pub fn get(&self, locale: &str) -> Option<&str> {
        self.0.get(locale).map(|s| s.as_str())
    }

    /// 是否没有任何可用翻译.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// 去重后按语言标签排序的条目列表.
    /// 
    /// 用于需要稳定输出顺序的场景 (如接口响应、测试断言).
    pub fn sorted_entries(&self) -> Vec<(&str, &str)> {
        let mut entries: Vec<(&str, &str)> = self
            .0
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        entries
    }
}

impl From<HashMap<String, String>> for LocalizedString {
    fn from(value: HashMap<String, String>) -> Self {
        Self::from_pairs(value)
    }
}

impl From<&str> for LocalizedString {
    /// 以默认语言 `zh-CN` 包装单个文本.
    fn from(value: &str) -> Self {
        Self::single(DEFAULT_LOCALE, value)
    }
}

impl From<String> for LocalizedString {
    /// 以默认语言 `zh-CN` 包装单个文本.
    fn from(value: String) -> Self {
        Self::single(DEFAULT_LOCALE, value)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_order_is_locale_then_default_then_any() {
        let s = LocalizedString::from_pairs([("zh-CN", "胡桃"), ("en", "Hu Tao")]);

        assert_eq!(s.get_with_fallback("en", "zh-CN"), Some("Hu Tao"));
        assert_eq!(s.get_with_fallback("ja", "zh-CN"), Some("胡桃"));
        assert_eq!(s.get_with_fallback("ja", "ko"), s.0.values().next().map(|v| v.as_str()));
    }

    #[test]
    fn empty_text_is_not_stored() {
        let mut s = LocalizedString::new();
        s.insert("en", "Hu Tao");
        s.insert("en", "");

        assert_eq!(s.get("en"), None);
        assert!(s.is_empty());
    }

    #[test]
    fn empty_localized_string_has_no_fallback() {
        let s = LocalizedString::new();
        assert_eq!(s.get_with_fallback("zh-CN", "zh-CN"), None);
    }

    #[test]
    fn serializes_transparently_as_map() {
        let s = LocalizedString::from_pairs([("zh-CN", "胡桃")]);
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, r#"{"zh-CN":"胡桃"}"#);

        let parsed: LocalizedString = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, s);
    }
}
