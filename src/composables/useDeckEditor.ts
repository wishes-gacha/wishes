import { ref } from 'vue';
import { listCards } from '../api/card';
import { CreateDeckRequest, DeckSummary } from '../api/decks';
import { EventTag, LocalizedString, Tag } from '../types';
import { DEFAULT_DATA_LOCALE } from './useLocaleText';

export function useDeckEditor() {
  const formData = ref<CreateDeckRequest>({
    name: {},
    members: { conditions: [] },
    event_groups: {},
    tags: [],
  });

  const allTags = ref<Tag[]>([]);
  const loadingTags = ref(false);

  // 加载所有卡片标签（用于标签选择器）
  const loadAllTags = async () => {
    loadingTags.value = true;
    try {
      const cards = await listCards();
      const tagMap = new Map<string, Tag>();
      cards.forEach(card => {
        card.tags.forEach(tag => {
          const key = `${tag.namespace}:${tag.value}`;
          if (!tagMap.has(key)) {
            tagMap.set(key, tag);
          }
        });
      });
      allTags.value = Array.from(tagMap.values());
    } finally {
      loadingTags.value = false;
    }
  };

  // 添加/删除活动组
  function addEventGroup(tag: EventTag) {
    if (!formData.value.event_groups[tag]) {
      formData.value.event_groups[tag] = { conditions: [] };
    }
  }

  function removeEventGroup(tag: EventTag) {
    delete formData.value.event_groups[tag];
  }

  /** 重置表单, 传入卡组时以其多语言名称作为初始值 */
  function resetForm(initial?: DeckSummary) {
    const name: LocalizedString = initial
      ? { ...initial.name }
      : { [DEFAULT_DATA_LOCALE]: '' };
    
    formData.value = {
      name,
      members: { conditions: [] }, // 需从后端获取完整数据，暂简化
      event_groups: {},
      tags: initial ? [...initial.tags] : [],
    };
  }

  return {
    formData,
    allTags,
    loadingTags,
    loadAllTags,
    addEventGroup,
    removeEventGroup,
    resetForm,
  };
}
