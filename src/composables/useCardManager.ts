import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { CardCreateRequest, CardSummary, CardUpdateRequest, createCard, deleteCard, listCards, setCardShadow, updateCard } from "../api/card";
import { Tag } from "../types";
import { allLocalizedValues, pickLocalized, useLocaleText } from "./useLocaleText";


export function useCardManager() {
  const cards = ref<CardSummary[]>([]);
  const loading = ref(false);

  /** 只保存 `loadCards` 的失败信息 */
  const error = ref<string | null>(null);

  const { t } = useI18n();
  const { currentLocale } = useLocaleText();

  const searchQuery = ref("");
  const selectedTagFilter = ref<Tag | null>(null);
  const filteredCards = computed(() => {
    let result = cards.value;

    // 文本搜索: 命中任意一种语言的文本都算匹配
    if (searchQuery.value.trim()) {
      const q = searchQuery.value.toLocaleLowerCase();
      result = cards.value.filter(card => 
        allLocalizedValues(card.content).some(text => text.toLocaleLowerCase().includes(q))
        || allLocalizedValues(card.title).some(text => text.toLocaleLowerCase().includes(q))
      )
    }

    return result
  })

  /** 卡片在当前界面语言下的显式内容 (缺失时回退到默认语言) */
  function displayContent(card: CardSummary): string {
    return pickLocalized(card.content, currentLocale.value);
  }

  async function loadCards() {
    loading.value = true;
    error.value = null;
    try {
      // Uuid v7 时间有序, 后端已按 global_id 升序排号 (相当于按创建时间排序)
      cards.value = await listCards();
    } catch (e) {
      error.value = t("card.loadFailed", { error: e });
    } finally {
      loading.value = false;
    }
  }

  async function create(req: CardCreateRequest): Promise<CardSummary> {
    loading.value = true;
    try {
      const newCard = await createCard(req);
      cards.value.unshift(newCard);
      return newCard;
    } finally {
      loading.value = false;
    }
  }

  async function update(req: CardUpdateRequest): Promise<CardSummary> {
    loading.value = true;
    try {
      const updated = await updateCard(req);
      await loadCards();
      // const index = cards.value.findIndex(c => c.global_id === req.global_id);
      // if (index !== -1) {
      //   cards.value[index] = updated;
      // } else {
      //   // 只读来源的卡片更新时会分叉出新的 global_id, 直接加入列表
      //   cards.value.unshift(updated);
      // }
      return updated;
    } finally {
      loading.value = false;
    }
  }

  async function remove(globalId: string): Promise<void> {
    loading.value = true;
    try {
      await deleteCard(globalId);
      await loadCards();    // 重新拉取, 在删除派生对象时让原对象重新显示
    } finally {             // 错误继续上抛, 由调用方处理
      loading.value = false;
    }
  }

  /** 设置卡片的遮蔽 */
  async function toggleShadow(globalId: string, shadowed: boolean): Promise<CardSummary> {
    loading.value = true;
    try {
      const updated = await setCardShadow(globalId, shadowed);
      await loadCards();
      return updated;
    } finally {
      loading.value = false;
    }
  }

  const allUsedTags = computed(() => {
    const tagMap = new Map<string, Tag>();
    for (const card of cards.value) {
      for (const tag of card.tags) {
        const key = `${tag.namespace}:${tag.value}`;
        if (!tagMap.has(key)) {
          tagMap.set(key, tag);
        }
      }
    }
    return Array.from(tagMap.values());
  })

  return {
    cards,
    loading,
    error,
    searchQuery,
    filteredCards,
    allUsedTags,
    selectedTagFilter,
    displayContent,
    loadCards,
    create,
    update,
    remove,
    toggleShadow,
  }
}
