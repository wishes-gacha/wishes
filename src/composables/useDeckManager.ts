import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { createDeck, CreateDeckRequest, DeckSummary, deleteDeck, listDecks, updateDeck, UpdateDeckRequest } from "../api/decks";
import { allLocalizedValues, pickLocalized, useLocaleText } from "./useLocaleText";


export function useDeckManager() {
  const decks = ref<DeckSummary[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);

  const { t } = useI18n();
  const { currentLocale } =  useLocaleText();

  const searchQuery = ref("");
  const filteredDecks = computed(() => {
    let result = decks.value;

    // 文本搜索: 命中任意一种语言的名称都算匹配
    if (searchQuery.value.trim()) {
      const q = searchQuery.value.toLocaleLowerCase();
      result = decks.value.filter(deck => 
        allLocalizedValues(deck.name).some(text => text.toLocaleLowerCase().includes(q))
      )
    }

    return result
  });

  /** 卡组在当前界面语言下的显示名 (缺失时回退到默认语言). */
  function displayName(deck: DeckSummary): string {
    return pickLocalized(deck.name, currentLocale.value);
  }

  // 获取卡片使用的所有标签可以直接使用 useCardManager 的 allUsedTags

  async function loadDecks() {
    loading.value = true;
    error.value = null;
    try {
      // Uuid v7 时间有序, 后端已排好
      decks.value = await listDecks();
    } catch (e) {
      error.value = t("deck.loadFailed", { error: e });
    } finally {
      loading.value = false;
    }
  }

  function getDeckById(globalId: string): DeckSummary | undefined {
    return decks.value.find(d => d.global_id === globalId);
  }

  async function create(req: CreateDeckRequest): Promise<DeckSummary | null> {
    loading.value = true;
    error.value = null;
    try {
      const newDeck = await createDeck(req);
      decks.value.unshift(newDeck);
      return newDeck
    } catch (e) {
      error.value = t("deck.createFailed", { error: e });
      return null;
    } finally {
      loading.value = false;
    }
  }

  async function update(req: UpdateDeckRequest): Promise<DeckSummary | null> {
    loading.value = true;
    error.value = null;
    try {
      const updated = await updateDeck(req);
      const index = decks.value.findIndex(d => d.global_id === req.global_id);
      if (index !== -1) {
        decks.value[index] = updated;
      } else {
        // 从只读来源分叉新 global_id
        decks.value.unshift(updated);
      }
      return updated;
    } catch (e) {
      error.value = t("deck.updateFailed", { error: e });
      return null;
    } finally {
      loading.value = false;
    }
  }

  async function remove(globalId: string): Promise<boolean> {
    loading.value = true;
    error.value = null;
    try {
      await deleteDeck(globalId);
      await loadDecks();
      // decks.value = decks.value.filter(d => d.global_id !== globalId);
      return true;
    } catch (e) {
      error.value = t("deck.deleteError", { error: e });
      return false;
    } finally {
      loading.value = false;
    }
  }

  return {
    decks,
    searchQuery,
    filteredDecks,
    loading,
    error,
    displayName,
    loadDecks,
    getDeckById,
    create,
    update,
    remove
  }
}
