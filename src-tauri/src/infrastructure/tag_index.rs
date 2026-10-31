//! # 标签索引
//! 
//! 提供基于 `Tag` 的泛型倒排索引, 支持插入和按标签集合查询 Id.

use std::collections::HashSet;
use dashmap::DashMap;
use crate::domain::tag::Tag;

/// 泛型标签索引.
/// 
/// 维护从 `Tag` 到一组 Id 的映射.
/// 适用于需要按标签快速检索对象的场景, 主要用于卡片、卡组、卡池、逻辑定义等.
/// 
/// 数据格式 v2 起, 实际使用的 `Id` 是 `GlobalId` (`Uuid`), 它不实现 `Copy`,
/// 因此泛型约束使用 `Clone`.
pub struct TagIndex<Id> {
    tag_to_ids: DashMap<Tag, HashSet<Id>>
}

impl<Id> Default for TagIndex<Id>
where
    Id: std::hash::Hash + Eq + Clone + Send + Sync
{
    fn default() -> Self {
        Self::new()
    }
}

impl<Id> TagIndex<Id>
where
    Id: std::hash::Hash + Eq + Clone + Send + Sync
{
    /// 创建空的标签索引.
    pub fn new() -> Self {
        Self { tag_to_ids: DashMap::new() }
    }
    
    /// 为指定 Id 插入一组标签.
    /// 
    /// 每个标签都会建立到该 Id 的映射.
    pub fn insert(&self, id: Id, tags: &[Tag]) {
        for tag in tags {
            self.tag_to_ids.entry(tag.clone()).or_default().insert(id.clone());
        }
    }

    pub fn remove(&self, id: Id, tag: &Tag) {
        if let Some(mut entry) = self.tag_to_ids.get_mut(tag) {
            entry.remove(&id);
            if entry.is_empty() {
                drop(entry);
                self.tag_to_ids.remove(tag);        // 集合空时删掉
            }
        }
    }

    /// 查询拥有所有给定标签集合的所有 Id (交集查询).
    /// 
    /// 若 `tags` 为空, 返回空集.
    pub fn query_all(&self, tags: &[Tag]) -> HashSet<Id> {
        let mut iter = tags.iter();
        if let Some(first) = iter.next() {
            let mut result = self.tag_to_ids.get(first)
                .map(|id| id.clone())
                .unwrap_or_default();
            for tag in iter {
                if let Some(entry) = self.tag_to_ids.get(tag) {
                    result.retain(|id| entry.contains(id));
                } else {
                    return HashSet::new();
                }
                if result.is_empty() { break; }
            }
            result
        } else {
            HashSet::new()
        }
    }

    /// 查询拥有任意一个给定标签的所有 Id (并集查询).
    /// 
    /// 若 `tags` 为空, 返回空集.
    pub fn query_any(&self, tags: &[Tag]) -> HashSet<Id> {
        let mut result = HashSet::new();
        for tag in tags {
            if let Some(entry) = self.tag_to_ids.get(tag) {
                result.extend(entry.value().iter().cloned());
            }
        }
        result
    }

    /// 返回索引中所有的 Id 集合.
    pub fn all_ids(&self) -> HashSet<Id> {
        let mut res = HashSet::new();
        for entry in self.tag_to_ids.iter() {
            res.extend(entry.value().iter().cloned());
        }
        res
    }
}