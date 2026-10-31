<script setup lang="ts">
import { computed, onMounted, ref, h } from 'vue';
import { useCardManager } from '../../composables/useCardManager';
import { CardCreateRequest, CardSummary, CardUpdateRequest, getCardReferencingDecks } from '../../api/card';
import type { DeckReference } from '../../api/card';
import { LocalizedString, Tag } from '../../types';
import CardForm from '../CardForm.vue';
import { useTagStyles } from '../../composables/useTagStyles.ts';
import { useMessage, useDialog } from 'naive-ui';
import { useI18n } from 'vue-i18n';
import { originLabel, shortGlobalId, useLocaleText } from '../../composables/useLocaleText.ts';

const {
  cards,
  loading,
  error,
  searchQuery,
  filteredCards,
  allUsedTags,
  displayContent,
  loadCards,
  create,
  update,
  remove,
  toggleShadow,
} = useCardManager();

const message = useMessage();
const dialog = useDialog();
const { t } = useI18n();
const { pickLocalized } = useLocaleText();

// 选中卡片
const selectedCard = ref<CardSummary | null>(null);

// 弹窗状态
const showModal = ref(false);
const isEditMode = ref(false);
const editingCard = ref<CardSummary | null>(null);
const formData = ref<CardCreateRequest>({
  content: {},
  title: {},
  tags: []
})

const shadowSource = ref(true);

const formValid = ref(false);

const deleting = ref(false);

// 标签筛选浮窗
const showTagFilterPopup = ref(false);
const selectedFilterTags = ref<Tag[]>([]);

// 标签筛选模式: 所有标签包含/任意标签包含
const tagFilterMode = ref<'AND' | 'OR'>('AND');

const tagGroups = computed(() => {
  const groupMap = new Map<string, Tag[]>();

  allUsedTags.value.forEach(tag => {
    if (!groupMap.has(tag.namespace)) {
      groupMap.set(tag.namespace, []);
    }
    groupMap.get(tag.namespace)!.push(tag);
  });

  const sortedGroups = Array.from(groupMap.entries()).sort((a, b) => a[0].localeCompare(b[0]));
  return sortedGroups.map(([namespace, tags]) => ({
    namespace,
    tags: tags.sort((a, b) => a.value.localeCompare(b.value))
  }));
})

// 过滤后的卡片
const finalFilteredCards = computed(() => {
  let result = filteredCards.value;

  if (selectedFilterTags.value.length > 0) {
    if (tagFilterMode.value === 'AND') {
      // 要求卡片包含所有选中标签
      result = result.filter(card =>
        selectedFilterTags.value.every(filterTag => 
          card.tags.some(t => t.namespace === filterTag.namespace && t.value === filterTag.value)
        )
      );
    } else {
      // 要求卡片至少包含一个选中的标签
      result = result.filter(card => 
        selectedFilterTags.value.some(filterTag =>
          card.tags.some(t => t.namespace === filterTag.namespace && t.value === filterTag.value)
        )
      );
    }
  }

  return result;
})

onMounted(() => {
  loadCards();
})

function selectCard(card: CardSummary) {
  selectedCard.value = card;
}

function clearSelected() {
  selectedCard.value = null;
}

function openCreateModal() {
  isEditMode.value = false;
  editingCard.value = null;
  formData.value = { content: {}, title: {}, tags: [] };
  formValid.value = false;
  showModal.value = true;
  shadowSource.value = true;
}

function openEditModal(card: CardSummary) {
  // 只读来源且有派生 → 引导编辑派生
  if (card.derived_id) {
    dialog.warning({
      title: t('card.derivedExistsTitle'),
      content: t('card.derivedExistsContent', { name: displayContent(card) }),
      positiveText: t('card.editDerivedAction'),
      negativeText: t('common.cancel'),
      onPositiveClick: () => {
        const derived = cards.value.find(c => c.global_id === card.derived_id);
        if (derived) {
          selectCard(derived);
          doOpenEditModal(derived);
        } else {
          message.warning(t('card.derivedNotFound'));
        }
      },
    });
    return;
  }
  doOpenEditModal(card);
}

function doOpenEditModal(card: CardSummary) {
  isEditMode.value = true;
  editingCard.value = card;
  formData.value = {
    content: { ...card.content },
    title: { ...(card.title ?? {}) },
    tags: [...card.tags],
  };
  formValid.value = Object.values(card.content).some(v => v.trim().length > 0);
  showModal.value = true;
  shadowSource.value = true;
}

function closeModal() {
  showModal.value = false;
}

function onFormChanged(data: { content: LocalizedString, title: LocalizedString, tags: Tag[], valid: boolean }) {
  formData.value = {
    content: data.content,
    title: data.title,
    tags: data.tags,
  };
  formValid.value = data.valid;
}

async function handleSave() {
  try {
    if (isEditMode.value && editingCard.value) {
      const req: CardUpdateRequest = {
        global_id: editingCard.value.global_id,
        content: formData.value.content,
        title: formData.value.title,
        tags: formData.value.tags,
        shadow: shadowSource.value,
      }
      const result = await update(req);
      closeModal();
      const refreshed = cards.value.find(c => c.global_id === result?.global_id);
      selectCard(refreshed ?? result);
    } else {
      const result = await create(formData.value);
      closeModal();
      selectCard(result);
    }
  } catch (e) {
    message.error(t(isEditMode.value ? "card.updateFailed" : "card.createFailed", { error: e }));
  }
}

async function toggleShadowAction(card: CardSummary) {
    try {
        const updated = await toggleShadow(card.global_id, !card.shadowed);
        message.success(t(updated.shadowed ? 'card.shadowEnabled' : 'card.shadowDisabled'));
        const refreshed = cards.value.find(c => c.global_id === updated.global_id);
        if (refreshed) selectCard(refreshed);
    } catch (e) {
        message.error(String(e));
    }
}

async function confirmDelete(card: CardSummary) {
  if (deleting.value) return;
  deleting.value = true;
  let refDecks: DeckReference[] = [];
  try {
    refDecks = await getCardReferencingDecks(card.global_id);
  } catch (e) {
    message.warning(t('card.refDecksLoadFailed', { error: e }));
  }

  const isFork = !!card.forked_from;
  const sourceName = card.forked_from_name
    ? pickLocalized(card.forked_from_name)
    : shortGlobalId(card.forked_from);
  
  const content = h('div', { class: 'wishes-delete-dialog' }, [
    h('p', { class: 'confirm-message' },
      t('card.deleteConfirmContent', { name: displayContent(card) })
    ),

    isFork && h('div', { class: 'fork-visibility-hint' }, [
      h('p', {}, t('card.deleteForkRestore', { name: sourceName })),
    ]),

    refDecks.length > 0 && h('div', { class: 'confirm-refs' }, [
      h('p', { class: 'ref-hint' },
        isFork
          ? t('card.deleteRefDeckForkInfo')
          : t('card.deleteRefDeckWarning')
      ),
      h('ul', { class: 'ref-list' },
        refDecks.map(d => h('li', { key: d.global_id }, pickLocalized(d.name)))
      ),
    ]),

    h('p', { class: 'warning' }, t('card.deleteWarning')),
  ]);

  dialog.warning({
    title: t('card.deleteConfirmTitle'),
    content: () => content,
    positiveText: t('card.deleteConfirmTitle'),
    negativeText: t('common.cancel'),
    positiveButtonProps: { color: '#e74c3c', textColor: '#fff' },
    onPositiveClick: async () => {
      try {
        await remove(card.global_id);
        if (selectedCard.value?.global_id === card.global_id) clearSelected();
        message.success(t('card.deleteSuccess'));
      } catch (e) {
        message.error(t("card.deleteFailed", { error: e }));
        return false;
      }
    }
  })

  deleting.value = false;
}

// function closeDeleteConfirm() {
//   showDeleteConfirm.value = false;
//   deletingCard.value = null;
//   referencingDecks.value = [];
// }

// async function handleDelete() {
//   if (!deletingCard.value) return;
//   const id = deletingCard.value.global_id;
//   try {
//     await remove(id);
//     if (selectedCard.value?.global_id === id) clearSelected();
//     closeDeleteConfirm();
//     message.success(t('card.deleteSuccess'))
//   } catch (e) {
//     message.error(t("card.deleteFailed", { error: e }));
//   }
// }

// 标签筛选相关
function toggleTagFilter(tag: Tag) {
  const index = selectedFilterTags.value.findIndex(
    t => t.namespace === tag.namespace && t.value === tag.value
  );
  if (index !== -1) {
    selectedFilterTags.value.splice(index, 1);
  } else {
    selectedFilterTags.value.push(tag);
  }
}

function clearTagFilters() {
  selectedFilterTags.value = [];
}

function closeTagFilterPopup() {
  showTagFilterPopup.value = false;
}

const selectedCardTagClasses = computed(() => {
  if (!selectedCard.value) return []
  return useTagStyles(selectedCard.value.tags);
})
</script>


<template>
  <div class="manager-card">
    <div class="header-actions">
      <div class="search-wrapper">
        <input 
          v-model="searchQuery"
          :placeholder="t('card.searchPlaceholder')"
          class="search-input"
        />
        <div class="filtered-card-counter">
          {{ t('card.totalCounter', { filtered: finalFilteredCards.length, total: cards.length }) }}
        </div>
        <div class="filter-tags" @click="showTagFilterPopup = !showTagFilterPopup">
          <span v-if="selectedFilterTags.length === 0" class="filter-placeholder">{{ t('card.filterTags') }}</span>
          <span v-else class="filter-active">
            {{ t('card.activeFilterTags', { count: selectedFilterTags.length }) }}
          </span>
          <span class="filter-arrow">▼</span>
          
        </div>
        <!-- 标签筛选弹窗 -->
        <div v-if="showTagFilterPopup" class="tag-filter-popup" @click.stop>
          <div class="filter-popup-header">
            <span>{{ t('card.filterPopupHint') }}</span>
            <div class="filter-mode-toggle">
              <button
                class="mode-btn"
                :class="{ active: tagFilterMode === 'AND' }"
                @click="tagFilterMode = `AND`"
              >{{ t('card.filterModeAnd') }}</button>
              <button
                class="mode-btn"
                :class="{ active: tagFilterMode === 'OR' }"
                @click="tagFilterMode = 'OR'"
              >{{ t('card.filterModeOr') }}</button>
            </div>
            <button class="filter-clear" @click="clearTagFilters">{{ t('card.clear') }}</button>
          </div>
          <div class="filter-popup-tags">
            <div v-for="group in tagGroups" :key="group.namespace" class="filter-tag-group">
              <span class="filter-group-label">{{ group.namespace }}</span>
              <div class="filter-group-tags">
                <span
                  v-for="tag in group.tags"
                  :key="`${tag.namespace}:${tag.value}`"
                  class="filter-tag"
                  :class="{ active: selectedFilterTags.some(t => t.namespace === tag.namespace && t.value === tag.value) }"
                  @click="toggleTagFilter(tag)"
                >
                  {{ tag.value }}
                </span>
              </div>
            </div>
          </div>
          <div class="filter-popup-footer">
            <button class="filter-close" @click="closeTagFilterPopup">{{ t('card.confirm') }}</button>
          </div>
        </div>
        
      </div>
      <button class="btn-create" @click="openCreateModal">{{ t('card.create') }}</button>
    </div>

    <!-- 主体两栏 -->
    <div class="main-body">
      <!-- 左侧: 卡片列表 -->
      <div class="card-list">
        <div v-if="loading" class="loading-state">{{ t('common.loading') }}</div>
        <div v-else-if="error && cards.length === 0" class="error-state">{{ error }}</div>
        <div v-else-if="finalFilteredCards.length === 0" class="empty-state">
          <p>{{ t('card.noMatch') }}</p>
        </div>
        <div v-else class="card-grid">
          <div
            v-for="card in finalFilteredCards"
            :key="card.global_id"
            class="card-item"
            :class="{ active: selectedCard?.global_id === card.global_id }"
            @click="selectCard(card)"
          >
            <div class="card-preview">
              <div class="card-thumbnail">
                <div class="card-thumbnail-placeholder"></div>
              </div>
              <div class="card-content-preview">{{ displayContent(card) }}</div>
              <div class="card-tags-preview">
                <span
                  v-for="tag in card.tags.slice(0, 3)"
                  :key="`${tag.namespace}:${tag.value}`"
                  class="tag"
                >
                  {{ tag.value }}
                </span>
                <span v-if="card.tags.length > 3" class="tag-more">+{{ card.tags.length - 3 }}</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- 右侧: 详情 -->
      <div class="card-detail">
        <div v-if="!selectedCard" class="detail-empty">
          <span class="empty-icon">👆</span>
          <span>{{ t('card.detailEmpty') }}</span>
        </div>
        <div v-else class="detail-content">
          <div class="detail-image">
            <div class="detail-image-placeholder"></div>
          </div>
          <div class="detail-id-name">
            <div class="detail-name">{{ displayContent(selectedCard) }}</div>
          </div>
          <div class="detail-origin">
            <span class="origin-badge" :class="`origin-${selectedCard.origin}`">
              {{ originLabel(t, selectedCard.origin) }}
            </span>
            <span v-if="selectedCard.forked_from" class="fork-hint">
              {{ t('card.forkedFrom', { name: pickLocalized(selectedCard.forked_from_name) || shortGlobalId(selectedCard.forked_from) }) }}
            </span>
            <span v-if="selectedCard.origin !== 'local'" class="fork-edit-hint">
              {{ t('card.forkEditHint') }}
            </span>
          </div>
          <div v-if="selectedCard.forked_from" class="shadow-status">
            <span class="shadow-badge" :class="selectedCard.shadowed ? 'shadow-on' : 'shadow-off'">
              {{ selectedCard.shadowed ? t('card.shadowedYes') : t('card.shadowedNo') }}
            </span>
            <button class="btn-toggle-shadow" @click="toggleShadowAction(selectedCard)">
              {{ selectedCard.shadowed ? t('card.unshadowAction') : t('card.shadowAction') }}
            </button>
          </div>
          <div class="detail-tags">
            <span
              v-for="(tag, index) in selectedCard.tags"
              :key="`${tag.namespace}:${tag.value}`"
              class="detail-tag"
              :class="selectedCardTagClasses[index]"
              >
              {{ tag.namespace }}: {{ tag.value }}
            </span>
          </div>
          <div class="detail-actions">
            <button
              v-if="selectedCard.derived_id"
              class="btn-detail-edit"
              @click="openEditModal(selectedCard)"
            >{{ t('card.editDerivedAction') }}</button>
            <button
              v-else
              class="btn-detail-edit"
              @click="openEditModal(selectedCard)"
            >{{ t('card.edit') }}</button>
            <!-- <button class="btn-detail-edit" @click="openEditModal(selectedCard)">{{ t('card.edit') }}</button> -->
            <button v-if="selectedCard.origin === 'local'" class="btn-detail-delete" @click="confirmDelete(selectedCard)">{{ t('card.delete') }}</button>
          </div>
        </div>
      </div>
    </div>

    <!-- 创建/编辑弹窗 -->
    <div v-if="showModal" class="modal-overlay" @click="closeModal">
      <div class="modal-content" @click.stop>
        <div class="modal-header">
          <h3>{{ isEditMode ? t('card.editTitle') : t('card.createTitle') }}</h3>
          <button class="modal-close" @click="closeModal">✕</button>
        </div>
        <div class="modal-body">
          <CardForm
            :initial-content="editingCard?.content"
            :initial-title="editingCard?.title"
            :initial-tags="editingCard?.tags"
            :existing-tags="allUsedTags"
            @change="onFormChanged"
          />
        </div>
        <div class="modal-footer">
          <label v-if="isEditMode && editingCard?.origin !== 'local'" class="shadow-option">
            <input type="checkbox" v-model="shadowSource" />
            <div class="shadow-option-text">
              <span>{{ t('card.shadowSourceLabel') }}</span>
              <span class="shadow-option-hint">{{ t('card.shadowSourceHint') }}</span>
            </div>
          </label>
          <button class="btn-cancel" @click="closeModal">{{ t('common.cancel') }}</button>
          <button class="btn-save" @click="handleSave" :disabled="!formValid">
            {{ isEditMode ? t('card.save') : t('card.createAction') }}
          </button>
        </div>
      </div>
    </div>
  </div>
</template>


<style scoped>
.manager-card {
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

/* 顶部搜索栏 */
.header-actions {
  display: flex;
  gap: 12px;
  align-items: center;
  flex-shrink: 0;
  padding-bottom: 12px;
  border-bottom: 1px solid #2a3340;
  flex-wrap: wrap;
}

.search-wrapper {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 8px;
  position: relative;
}

.search-input {
  flex: 1;
  border: none;
  border-bottom: 2px solid #2a3340;
  background: #1a212b1c;
  /* border: 1px solid #2a3340; */
  /* border-radius: 8px; */
  padding: 6px 8px;
  font-size: 1.2rem;
  min-width: 180px;
  color: #e0e3e7;
}
.search-input:focus {
  outline: none;
  border-color: #007bff;
}
.filtered-card-counter {
  background: #1a212b;
  border: 1px solid #2a3340;
  border-radius: 8px;
  padding: 8px 14px;
  font-size: 1.1rem;
  white-space: nowrap;
  user-select: none;
}
.filter-tags {
  background: #1a212b;
  border: 1px solid #2a3340;
  border-radius: 8px;
  padding: 8px 14px;
  color: #b0c0d0;
  cursor: pointer;
  display: flex;
  align-items: center;
  gap: 8px;
  white-space: nowrap;
  font-size: 1.1rem;
  user-select: none;
}
.filter-tags:hover {
  border-color: #007bff;
}
.filter-placeholder {
  color: #6a7a90;
}
.filter-active {
  color: #007bff;
}
.filter-arrow {
  font-size: 0.7rem;
}

.btn-create {
  background: #007bff;
  border: none;
  border-radius: 8px;
  padding: 8px 20px;
  color: #fff;
  font-weight: 600;
  cursor: pointer;
  font-size: 1.1rem;
  transition: background 0.2s;
  white-space: nowrap;
}
.btn-create:hover {
  background: #3d9bff;
}

/* 标签筛选浮窗 */
.tag-filter-popup {
  position: absolute;
  top: calc(100% + 8px);
  right: 0;
  background: #1a212b80;
  backdrop-filter: blur(8px) saturate(150%);
  -webkit-backdrop-filter: blur(8px) saturate(150%);
  border: 1px solid #2a3340;
  border-radius: 12px;
  padding: 16px;
  z-index: 50;
  box-shadow: 0 12px 30px rgba(0,0,0,0.5);
  max-height: 280px;
  max-width: 680px;
  width: 100%;
  /* width: calc(100%); */
  display: flex;
  flex-direction: column;
  user-select: none;
}
.filter-popup-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 12px;
}
.filter-popup-header span {
  color: #b0c0d0;
  font-size: 1.2rem;
}
.filter-mode-toggle {
  display: flex;
  gap: 4px;
  background: #2a3340;
  border-radius: 6px;
  padding: 2px;
}
.mode-btn {
  background: transparent;
  border: none;
  color: #b0c0d0;
  padding: 4px 12px;
  border-radius: 4px;
  font-size: 1.1rem;
  cursor: pointer;
  transition: all 0.2s;
}
.mode-btn.active {
  background: #007bff;
  color: #fff;
}
.mode-btn:hover:not(.active) {
  background: #3a4a5a;
}
.filter-clear {
  background: none;
  border: none;
  color: #007bff;
  cursor: pointer;
  font-size: 1.2rem;
}
.filter-popup-tags {
  display: flex;
  flex-direction: column;
  gap: 6px;
  overflow-y: auto;
  max-height: 180px;
  padding: 4px 0;
}
.filter-popup-tags::-webkit-scrollbar {
  width: 4px;
}
.filter-popup-tags::-webkit-scrollbar-thumb {
  background: #2a3340;
  border-radius: 4px;
}
.filter-tag-group {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.filter-group-label {
  font-size: 1.3rem;
  font-weight: bold;
  color: #6e809a;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}
.filter-group-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  padding-left: 4px;
}
.filter-tag {
  background: rgba(255,255,255,0.06);
  color: #b0c0d0;
  padding: 4px 12px;
  border-radius: 16px;
  font-size: 1rem;
  cursor: pointer;
  transition: all 0.2s;
}
.filter-tag:hover {
  background: rgba(79, 110, 247, 0.2);
}
.filter-tag.active {
  background: #007bff;
  color: #fff;
}
.filter-popup-footer {
  margin-top: 12px;
  display: flex;
  justify-content: flex-end;
}
.filter-close {
  background: #007bff;
  border: none;
  border-radius: 8px;
  padding: 6px 20px;
  color: #fff;
  cursor: pointer;
  font-size: 1.1rem;
  user-select: none;
}

/* --- 主体两栏 --- */
.main-body {
  flex: 1;
  display: flex;
  gap: 4px;
  overflow: hidden;
  padding-top: 12px;
}

/* 左侧卡片列表 */
.card-list {
  /* flex: 0 0 60%; */
  flex: 1;
  overflow-y: auto;
  padding-right: 4px;
  scrollbar-width: thin;
  scrollbar-color: #2a3340 transparent;
}
.card-list::-webkit-scrollbar {
  width: 4px;
}
.card-list::-webkit-scrollbar-thumb {
  background: #2a3340;
  border-radius: 4px;
}

.card-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: 12px;
  padding-top: 4px;
}

.card-item {
  background: rgba(26, 33, 43, 0.25);
  border-radius: 12px;
  padding: 10px;
  border: 1px solid #2a3340;
  border-bottom: 5px solid #2a3340;
  cursor: pointer;
  transition: all 0.2s;
  max-width: 300px;
  user-select: none;
}
.card-item:hover {
  border-color: #3395ff;
  transform: translateY(-3px);
}
.card-item.active {
  border-color: #3395ff;
  background: rgba(79, 110, 247, 0.08);
}
.card-thumbnail {
  aspect-ratio: 3/4;
  border-radius: 8px;
  overflow: hidden;
  margin-bottom: 8px;
}
.card-thumbnail-placeholder {
  width: 100%;
  height: 100%;
  background: linear-gradient(135deg, #2a334080, #1a212b80);
}
.card-content-preview {
  text-align: center;
  font-size: 1.3rem;
  font-weight: 500;
  margin-bottom: 6px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: #eef1f4;
}
.card-tags-preview {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.card-tags-preview .tag {
  background: rgba(255,255,255,0.06);
  padding: 1px 8px;
  border-radius: 10px;
  font-size: 1rem;
  color: #8a99b4;
}
.card-tags-preview .tag-more {
  font-size: 1rem;
  color: #6a7a90;
}

/* 右侧详情 */
.card-detail {
  flex: 0 0 46%;
  max-width: 400px;
  background: #1a212b80;
  border-radius: 12px;
  border: 1px solid #2a3340;
  padding: 12px;
  /* overflow-y: auto; */
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
.detail-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  color: #6a7a90;
  gap: 8px;
}
.detail-empty .empty-icon {
  font-size: 2.5rem;
}
.detail-content {
  flex: 1;
  display: flex;
  flex-direction: column;
}
.detail-image {
  width: 100%;
  height: 50%;
  border-radius: 12px;
  overflow: hidden;
  margin-bottom: 8px;
}
.detail-image-placeholder {
  width: 100%;
  height: 100%;
  background: linear-gradient(135deg, #2a334080, #1a212b80);
}
.detail-id-name {
  display: flex;
  align-items: baseline;
}
.detail-name {
  flex: 1;
  text-align: center;
  font-size: 1.8rem;
  font-weight: bold;
  margin-bottom: 16px;
  word-break: break-word;
}
.detail-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-bottom: 24px;
  justify-content: center;
}
.detail-tag {
  background: var(--tag-bg, var(--tag-default-bg));
  color: var(--tag-color, var(--tag-default-color));

  padding: 4px 14px;
  border-radius: 16px;
  font-size: 1rem;
}

/* 来源与分叉信息 */
.detail-origin {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: center;
  gap: 8px;
  margin-bottom: 12px;
  font-size: 1.2rem;
}
.origin-badge {
  padding: 1px 8px;
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.08);
  color: #8a99b4;
  font-size: 1.3rem;
}
.origin-official {
  background: rgba(79, 110, 247, 0.2);
  color: #9db2ff;
}
.origin-local {
  background: rgba(80, 200, 120, 0.18);
  color: #7fd8a0;
}
.fork-hint {
  color: #6a7a90;
}
.fork-edit-hint {
  color: #d0a24c;
}

.shadow-status {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  margin-bottom: 12px;
}
.shadow-badge {
  padding: 2px 10px;
  border-radius: 10px;
  font-size: 1.1rem;
}
.shadow-on {
  background: rgba(79, 110, 247, 0.2);
  color: #9db2ff;
}
.shadow-off {
  background: rgba(255, 255, 255, 0.08);
  color: #8a99b4;
}
.btn-toggle-shadow {
  background: #3b4b5e;
  border: none;
  border-radius: 4px;
  padding: 3px 14px;
  color: #fff;
  font-size: 1.1rem;
  cursor: pointer;
  font-weight: 500;
}
.btn-toggle-shadow:hover {
  background: #4f6ef7;
}

.detail-actions {
  display: flex;
  flex-direction: row;
  gap: 10px;
  margin-top: auto;
  justify-content: flex-end;
}

.btn-detail-edit,
.btn-detail-delete {
  border: none;
  border-radius: 4px;
  padding: 4px 14px;
  font-size: 1.4rem;
  cursor: pointer;
  transition: background 0.1s, transform 0.1s;
  white-space: nowrap;
  font-weight: bold;
}
.btn-detail-edit {
  background: #3b4b5e;
  color: #fff;
}
.btn-detail-edit:hover {
  background: #4f6ef7;
}

.btn-detail-delete {
  background: #212830;
  border: 1px solid #3d444d;
  color: #fa5e55;
}
.btn-detail-delete:hover {
  background: #b62324;
  color: #ffe9ca;
}

/* --- 弹窗样式 --- */
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.7);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 200;
}
.modal-content {
  background: #1a212b80;
  border-radius: 16px;
  padding: 24px;
  max-width: 560px;
  width: 90%;
  border: 1px solid #2a3340;
  max-height: 90vh;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
  flex-shrink: 0;
}
.modal-header h3 {
  font-size: 2rem;
}
.modal-close {
  background: none;
  border: none;
  color: #6a7a90;
  font-size: 1.5rem;
  cursor: pointer;
  user-select: none;
}
.modal-close:hover {
  color: #e4e8ef;
}
.modal-body {
  flex: 1;
  overflow-y: auto;
  padding-right: 4px;
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
/* .modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 20px;
  flex-shrink: 0;
} */
.modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
  margin-top: 20px;
  flex-shrink: 0;
  align-items: center;   /* 让 checkbox 和按钮垂直居中 */
}
.shadow-option {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-right: auto;   /* 推到左侧 */
  cursor: pointer;
  user-select: none;
}
.shadow-option input[type="checkbox"] {
  width: 16px;
  height: 16px;
  cursor: pointer;
  accent-color: #007bff;
}
.shadow-option-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.shadow-option-text span:first-child {
  font-size: 1.2rem;
  color: #b0c0d0;
}
.shadow-option-hint {
  font-size: 1rem;
  color: #6a7a90;
}
.btn-cancel {
  background: transparent;
  border: 1px solid #2d3642;
  border-radius: 8px;
  padding: 8px 20px;
  color: #b0c0d0;
  cursor: pointer;
  user-select: none;
}
.btn-cancel:hover {
  background: #2d3642;
}
.btn-save {
  background: #007bff;
  border: none;
  border-radius: 8px;
  padding: 8px 24px;
  color: #fff;
  font-weight: 600;
  cursor: pointer;
  user-select: none;
}
.btn-save:hover {
  background: #3d9bff;
}
.btn-save:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}
.btn-danger {
  background: #e74c3c;
  border: none;
  border-radius: 8px;
  padding: 8px 24px;
  color: #fff;
  font-weight: bold;
  cursor: pointer;
}
.btn-danger:hover {
  background: #ff6b6b;
}
.confirm-dialog h3 {
  font-size: 2rem;
}
.confirm-message {
  margin: 8px 0;
  font-size: 1.3rem;
}
.warning {
  color: #e74c3c;
  font-size: 1.3rem;
}
.confirm-refs {
  margin: 8px 0;
  padding: 8px 12px;
  border: 1px solid #2a3340;
  border-radius: 6px;
  background: rgba(231, 76, 60, 0.08);
}
.ref-hint {
  color: #e0a96d;
  font-size: 1.2rem;
  margin-bottom: 4px;
}
.ref-list {
  margin: 0;
  padding-left: 18px;
  color: #e4e8ef;
  font-size: 1.2rem;
  max-height: 12rem;
  overflow-y: auto;
}
.ref-list li {
  margin: 2px 0;
}

.loading-state,
.error-state,
.empty-state {
  text-align: center;
  padding: 50px 0;
  color: #6a7a90;
  font-size: 1.3rem;
}
</style>


<style>
/* 删除确认弹窗样式：dialog 内容传送到 body，scoped 不生效，用独立前缀隔离 */
.wishes-delete-dialog .confirm-message {
  margin: 8px 0;
  font-size: 1.3rem;
}
.wishes-delete-dialog .fork-visibility-hint {
  color: #d0a24c;
  font-size: 1.2rem;
  margin: 8px 0;
}
.wishes-delete-dialog .confirm-refs {
  margin: 8px 0;
  padding: 8px 12px;
  border: 1px solid #2a3340;
  border-radius: 6px;
  background: rgba(230, 44, 23, 0.178);
}
.wishes-delete-dialog .ref-hint {
  color: #e0a96d;
  font-size: 1.2rem;
  margin-bottom: 4px;
}
.wishes-delete-dialog .ref-list {
  margin: 0;
  padding-left: 18px;
  color: #e4e8ef;
  font-size: 1.2rem;
  max-height: 12rem;
  overflow-y: auto;
}
.wishes-delete-dialog .ref-list li { margin: 2px 0; }
.wishes-delete-dialog .warning {
  color: #e74c3c;
  font-size: 1.3rem;
  margin-top: 12px;
}
</style>