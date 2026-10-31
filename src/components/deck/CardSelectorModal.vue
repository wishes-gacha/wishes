<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { useI18n } from 'vue-i18n';
import { listCards } from '../../api/card';
import type { CardSummary } from '../../api/card';
import { allLocalizedValues, pickLocalized, useLocaleText } from '../../composables/useLocaleText';

const props = defineProps<{
  initialSelectedIds: string[];
}>();

const emit = defineEmits<{
  (e: 'confirm', ids: string[]): void;
  (e: 'close'): void;
}>();

const { t } = useI18n();
const { currentLocale } = useLocaleText();

const cards = ref<CardSummary[]>([]);
const searchQuery = ref('');
const selectedIds = ref<Set<string>>(new Set(props.initialSelectedIds));
const loading = ref(false);

/** 空格分隔的关键词, 全部小写 */
const searchTokens = computed<string[]>(() => {
  const q = searchQuery.value.trim().toLowerCase();
  if (!q) return [];
  return q.split(/\s+/).filter(Boolean);
});

/** 单个关键词是否命中卡片: 匹配任意语言的内容 / 称号 / 标签命名空间 / 标签值 */
function matchesToken(card: CardSummary, token: string): boolean {
  if (allLocalizedValues(card.content).some(text => text.toLowerCase().includes(token))) return true;
  return card.tags.some(tag =>
    tag.namespace.toLowerCase().includes(token) ||
    tag.value.toLowerCase().includes(token)
  );
}

/** 过滤: 所有关键词都命中才算匹配 (AND) */
const filteredCards = computed(() => {
  const tokens = searchTokens.value;
  if (tokens.length === 0) return cards.value;
  return cards.value.filter(card =>
    tokens.every(token => matchesToken(card, token))
  );
});

/** 排序: 已选优先, 其余保持后端已有顺序 */
const sortedCards = computed(() => {
  return [...filteredCards.value].sort((a, b) => {
    const aSelected = selectedIds.value.has(a.global_id);
    const bSelected = selectedIds.value.has(b.global_id);
    if (aSelected !== bSelected) return aSelected ? -1 : 1;
    return a.global_id.localeCompare(b.global_id);
  });
});

/** 高亮片段 */
interface Segment { text: string; matched: boolean }

function highlight(text: string, tokens: string[]): Segment[] {
  if (!text) return [{ text: '', matched: false }];
  if (tokens.length === 0) return [{ text, matched: false }];

  const lower = text.toLowerCase();
  const ranges: Array<[number, number]> = [];

  // 每个关键词的所有出现位置
  for (const token of tokens) {
    let from = 0;
    while (from < lower.length) {
      const idx = lower.indexOf(token, from);
      if (idx === -1) break;
      ranges.push([idx, idx + token.length]);
      from = idx + token.length;
    }
  }

  if (ranges.length === 0) return [{ text, matched: false }];

  // 合并重叠区间
  ranges.sort((a, b) => a[0] - b[0]);
  const merged: Array<[number, number]> = [];
  for (const [s, e] of ranges) {
    const last = merged[merged.length - 1];
    if (last && s <= last[1]) {
      last[1] = Math.max(last[1], e);
    } else {
      merged.push([s, e]);
    }
  }

  // 切成普通片段 / 高亮片段
  const segments: Segment[] = [];
  let cursor = 0;
  for (const [s, e] of merged) {
    if (cursor < s) segments.push({ text: text.slice(cursor, s), matched: false });
    segments.push({ text: text.slice(s, e), matched: true });
    cursor = e;
  }
  if (cursor < text.length) {
    segments.push({ text: text.slice(cursor), matched: false });
  }
  return segments;
}

/** 卡片在当前界面语言下的显式内容 */
const displayContent = (card: CardSummary) =>
  pickLocalized(card.content, currentLocale.value);

const contentSegments = (card: CardSummary) =>
  highlight(displayContent(card), searchTokens.value);

const tagSegments = (value: string) =>
  highlight(value, searchTokens.value);

onMounted(async () => {
  loading.value = true;
  try {
    cards.value = await listCards();
  } catch (e) {
    console.error('加载卡片失败', e);
  } finally {
    loading.value = false;
  }
});

function toggleCard(globalId: string) {
  if (selectedIds.value.has(globalId)) {
    selectedIds.value.delete(globalId);
  } else {
    selectedIds.value.add(globalId);
  }
}

function clearSearch() {
  searchQuery.value = '';
}

function confirm() {
  emit('confirm', Array.from(selectedIds.value));
}

function close() {
  emit('close');
}
</script>


<template>
  <div class="modal-overlay" @click="close">
    <div class="modal-content" @click.stop>
      <div class="modal-header">
        <h3>{{ t('condition.selectCards') }}</h3>
        <button class="close-btn" @click="close">✕</button>
      </div>

      <div class="search-bar">
        <div class="search-input-wrapper">
          <input
            v-model="searchQuery"
            :placeholder="t('condition.searchPlaceholder')"
            class="search-input"
          />
          <button
            v-if="searchQuery"
            class="search-clear"
            :title="t('condition.clearSearch')"
            @click="clearSearch"
          >✕</button>
        </div>
        <span class="selected-count">{{ t('condition.selectedCount', { count: selectedIds.size }) }}</span>
      </div>

      <div class="modal-body">
        <div v-if="loading" class="loading">{{ t('common.loading') }}</div>
        <div v-else class="card-grid">
          <div
            v-for="card in sortedCards"
            :key="card.global_id"
            class="card-item"
            :class="{ selected: selectedIds.has(card.global_id) }"
            @click="toggleCard(card.global_id)"
          >
            <div class="card-content">
              <span
                v-for="(seg, i) in contentSegments(card)"
                :key="i"
                :class="{ hl: seg.matched }"
              >{{ seg.text }}</span>
            </div>
            <div class="card-tags">
              <span
                v-for="tag in card.tags.slice(0, 2)"
                :key="`${tag.namespace}:${tag.value}`"
                class="tag"
              >
                <span
                  v-for="(seg, i) in tagSegments(tag.value)"
                  :key="i"
                  :class="{ hl: seg.matched }"
                >{{ seg.text }}</span>
              </span>
            </div>
            <div class="check-mark" v-if="selectedIds.has(card.global_id)">✓</div>
          </div>
          <div v-if="sortedCards.length === 0 && !loading" class="empty">
            {{ t('card.noMatch') }}
          </div>
        </div>
      </div>

      <div class="modal-footer">
        <button class="btn-cancel" @click="close">{{ t('common.cancel') }}</button>
        <button class="btn-confirm" @click="confirm">{{ t('condition.confirmSelection') }}</button>
      </div>
    </div>
  </div>
</template>


<style scoped>
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0,0,0,0.7);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 300;
}
.modal-content {
  background: #1a212b80;
  border-radius: 16px;
  padding: 20px;
  width: 90%;
  max-width: 700px;
  max-height: 80vh;
  min-height: 60vh;
  display: flex;
  flex-direction: column;
  border: 1px solid #2a3340;
}
.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  flex-shrink: 0;
}
.modal-header h3 {
  font-size: 2rem;
  margin: 0;
}
.close-btn {
  background: none;
  border: none;
  color: #6a7a90;
  font-size: 1.8rem;
  cursor: pointer;
}
.close-btn:hover {
  color: #fff;
}
.search-bar {
  margin-top: 6px;
  display: flex;
  gap: 8px;
  align-items: center;
  margin-bottom: 12px;
}
.search-input-wrapper {
  flex: 1;
  position: relative;
  display: flex;
  align-items: center;
}
.search-input {
  width: 100%;
  background: #0b0e14;
  border: none;
  border-bottom: 2px solid #2a3340;
  padding: 4px 24px 4px 8px; /* 右侧留出清空按钮 */
  color: #e4e8ef;
  font-size: 1.2rem;
}
.search-input:focus {
  outline: none;
  border-color: #007bff;
}
.search-clear {
  position: absolute;
  right: 2px;
  top: 50%;
  transform: translateY(-50%);
  background: none;
  border: none;
  color: #6a7a90;
  cursor: pointer;
  font-size: 1rem;
  padding: 2px 6px;
  line-height: 1;
}
.search-clear:hover {
  color: #fff;
}

/* 搜索高亮 */
.hl {
  background: rgba(0, 123, 255, 0.35);
  color: #fff;
  border-radius: 2px;
  padding: 0 1px;
}

.selected-count {
  color: #8a99b4;
  font-size: 1.1rem;
}
.modal-body {
  flex: 1;
  overflow-y: auto;
  margin: 6px 0;
  padding-right: 2px;
  scrollbar-width: thin;
  scrollbar-color: #2a3340 transparent;
}
.modal-body::-webkit-scrollbar {
  width: 4px;
}
.modal-body::-webkit-scrollbar-thumb {
  background: #2a3340;
  border-radius: 4px;
}

.card-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  gap: 8px;
}
.card-item {
  background: rgba(255,255,255,0.04);
  border-radius: 8px;
  padding: 8px;
  border: 2px solid transparent;
  cursor: pointer;
  position: relative;
  transition: border-color 0.2s, background 0.2s;
  user-select: none;
}
.card-item:hover {
  background: rgba(255,255,255,0.08);
}
.card-item.selected {
  border-color: #007bff;
  background: rgba(79, 110, 247, 0.15);
}
.card-content {
  /* font-weight: bold; */
  font-size: 1.3rem;
  margin-bottom: 4px;
}
.card-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.tag {
  background: rgba(255,255,255,0.06);
  padding: 0 6px;
  border-radius: 10px;
  font-size: 1rem;
  color: #8a99b4;
}
.check-mark {
  position: absolute;
  top: 4px;
  right: 6px;
  color: #007bff;
  font-size: 1.5rem;
  font-weight: bold;
}
.empty, .loading {
  text-align: center;
  color: #6a7a90;
  padding: 30px 0;
  font-size: 1.2rem;
}
.modal-footer {
  display: flex;
  gap: 12px;
  justify-content: flex-end;
  flex-shrink: 0;
  padding-top: 12px;
  border-top: 1px solid #2a3340;
}
.btn-confirm {
  background: #007bff;
  border: none;
  border-radius: 8px;
  padding: 8px 24px;
  color: #fff;
  font-weight: 600;
  cursor: pointer;
}
.btn-confirm:hover {
  background: #3d9bff;
}
.btn-cancel {
  background: transparent;
  border: 1px solid #2d3642;
  border-radius: 8px;
  padding: 8px 24px;
  color: #b0c0d0;
  cursor: pointer;
}
.btn-cancel:hover {
  background: #2d3642;
}
</style>