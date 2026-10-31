<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { useDeckManager } from '../../composables/useDeckManager';
import DeckEditor from '../deck/DeckEditor.vue';
import { useDialog, useMessage } from 'naive-ui';
import { originLabel, pickLocalized, shortGlobalId } from '../../composables/useLocaleText.ts';

const {
  decks,
  filteredDecks,
  searchQuery,
  loadDecks,
  create,
  update,        // 更新方法
  remove,        // 删除方法
  displayName,
  error,
} = useDeckManager();

const { t } = useI18n();

// 编辑状态
const editingDeckId = ref<string | null>(null);
const isCreating = ref(false);

const isEditorOpen = computed(() => editingDeckId.value !== null || isCreating.value);

const dialog = useDialog();
const message = useMessage();

const currentDeck = computed(() => {
  if (editingDeckId.value === null) return null;
  return decks.value.find(d => d.global_id === editingDeckId.value) ?? null;
});

// 开始编辑
function handleEdit(deckGlobalId: string) {
  isCreating.value = false;
  editingDeckId.value = deckGlobalId;
}

function handleCreate() {
  editingDeckId.value = null;
  isCreating.value = true;
}

// 保存编辑/新建
async function handleSave(data: any) {
  if (isCreating.value) {
    const result = await create({
      name: data.name,
      tags: data.tags,
      members: data.members,
      event_groups: data.event_groups,
    });
    if (result) {
      isCreating.value = false;
      await loadDecks();
    }
  } else if (editingDeckId.value !== null) {
    const result = await update({
      global_id: editingDeckId.value,
      name: data.name,
      tags: data.tags,
      members: data.members,
      event_groups: data.event_groups,
    });
    if (result) {
      editingDeckId.value = null;
      await loadDecks();
    }
  }
}

// 取消编辑
function handleCancel() {
  editingDeckId.value = null;
  isCreating.value = false;
}

// 删除确认（原有）
async function handleDelete(deckGlobalId: string) {
  dialog.warning({
    title: t('deck.deleteConfirmTitle'),
    content: t('deck.deleteConfirmContent'),
    positiveText: t('deck.deleteConfirmTitle'),
    negativeText: t('common.cancel'),
    onPositiveClick: async () => {
      const success = await remove(deckGlobalId);
      if (!success) {
        message.error(error.value ?? t('deck.deleteFailed'));
        return false;
      }
      message.success(t('deck.deleteSuccess'));
    },
    positiveButtonProps: {
      color: '#e74c3c',
      textColor: '#fff',
    }
  })
}

onMounted(() => {
  loadDecks();
})
</script>

<template>
  <div class="manager-deck-wrapper">
    <!-- 列表视图 -->
    <Transition name="slide-list">
      <div v-if="!isEditorOpen" class="deck-list-container">
        <!-- 顶部操作栏 -->
        <div class="header-actions">
          <div class="search-wrapper">
            <input 
              v-model="searchQuery"
              :placeholder="t('deck.searchPlaceholder')"
              class="search-input"
            />
            <div class="filtered-deck-counter">
              {{ t('deck.totalCounter', { filtered: filteredDecks.length, total: decks.length }) }}
            </div>
          </div>
          <button class="btn-create" @click="handleCreate">{{ t('deck.create') }}</button>
        </div>

        <div class="main-body">
          <div class="deck-list">
            <div
              v-for="deck in filteredDecks"
              :key="deck.global_id"
              class="deck-item"
            >
              <div class="deck-info">
                <div class="deck-name-info">
                  <div class="deck-name">{{ displayName(deck) }}</div>
                  <span class="origin-badge" :class="`origin-${deck.origin}`">
                    {{ originLabel(t, deck.origin) }}
                  </span>
                </div>
                <div v-if="deck.forked_from" class="deck-fork-hint">
                  {{ t('deck.forkedFrom', { name: pickLocalized(deck.forked_from_name) || shortGlobalId(deck.forked_from) }) }}
                </div>
                <div class="deck-tags">
                  <span v-if="deck.tags && deck.tags.length === 0" class="no-tags">{{ t('deck.noTags') }}</span>
                  <span
                    v-for="tag in deck.tags"
                    :key="`${tag.namespace}:${tag.value}`"
                    class="tag"
                  >
                    {{ tag.value }}
                  </span>
                </div>
              </div>
              <div class="deck-actions">
                <button class="action-btn edit-btn" @click="handleEdit(deck.global_id)">{{ t('deck.edit') }}</button>
                <button v-if="deck.origin === 'local'" class="action-btn delete-btn" @click="handleDelete(deck.global_id)">{{ t('deck.delete') }}</button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </Transition>

    <!-- 编辑视图 -->
    <Transition name="slide-editor">
      <div v-if="isEditorOpen" class="deck-editor-container">
        <DeckEditor
          :key="isCreating ? 'new' : (editingDeckId ?? 'none')"
          :deck="currentDeck ?? undefined"
          @save="handleSave"
          @cancel="handleCancel"
        />
      </div>
    </Transition>
  </div>
</template>

<style scoped>
/* 容器 */
.manager-deck-wrapper {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
}

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
  border-radius: 0 5px 0 0;
  border-bottom: 2px solid #2a3340;
  background: #1a212b1c;
  padding: 6px 8px;
  font-size: 1.2rem;
  min-width: 180px;
  color: #e0e3e7;
}
.search-input:focus {
  outline: none;
  border-color: #007bff;
}
.filtered-deck-counter {
  background: #1a212b;
  border: 1px solid #2a3340;
  border-radius: 8px;
  padding: 8px 14px;
  font-size: 1.1rem;
  white-space: nowrap;
  user-select: none;
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

.main-body {
  flex: 1;
  display: flex;
  gap: 4px;
  overflow: hidden;
  padding-top: 12px;
}

.deck-list {
  flex: 1;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 10px;
  scrollbar-width: thin;
  scrollbar-color: #2a3340 transparent;
}
.deck-list::-webkit-scrollbar {
  width: 4px;
}
.deck-list::-webkit-scrollbar-thumb {
  background: #2a3340;
  border-radius: 4px;
}

.deck-item {
  width: 100%;
  background: rgba(26, 33, 43, 0.4);
  border-radius: 10px;
  padding: 12px 18px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  transition: background 0.2s;
  user-select: none;
}
.deck-item:hover {
  background: rgba(26, 33, 43, 0.6);
}

/* 左侧信息区 */
.deck-info {
  flex: 0 0 30%;
  display: flex;
  flex-direction: column;
  gap: 12px;
  flex: 1;
  min-width: 0; /* 防止溢出 */
}

.deck-name-info {
  display: flex;
  flex-direction: row;
  gap: 12px;
  align-items: center;
}



.deck-name {
  font-size: 1.5rem;
  font-weight: 600;
  color: #e0e3e7;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.origin-badge {
  padding: 1px 8px;
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.08);
  color: #8a99b4;
  font-size: 1.3rem;
  white-space: nowrap;
}
.origin-official {
  background: rgba(79, 110, 247, 0.2);
  color: #9db2ff;
}
.origin-local {
  background: rgba(80, 200, 120, 0.18);
  color: #7fd8a0;
}

.deck-fork-hint {
  color: #6a7a90;
  font-size: 1.3rem;
}

.deck-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
  margin-left: 4px;
}

.tag {
  background: rgba(255,255,255,0.06);
  padding: 1px 8px;
  border-radius: 10px;
  font-size: 1.1rem;
  color: #8a99b4;
}

.no-tags {
  color: #6b7a8f;
  font-size: 1.1rem;
  font-style: italic;
}

/* 右侧操作按钮 */
.deck-actions {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-left: 16px;
  flex-shrink: 0;
}

.action-btn {
  border: none;
  border-radius: 4px;
  padding: 4px 14px;
  font-size: 1.2rem;
  cursor: pointer;
  transition: background 0.1s, transform 0.1s;
  white-space: nowrap;
  font-weight: bold;
}
.action-btn:active {
  transform: scale(0.95);
}

.edit-btn {
  background: #3b4b5e;
  color: #fff;
}
.edit-btn:hover {
  background: #4f6ef7;
}

.delete-btn {
  background: #212830;
  border: 1px solid #3d444d;
  color: #fa5e55;
}
.delete-btn:hover {
  background: #b62324;
  color: #ffe9ca;
}

.deck-list-container,
.deck-editor-container {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  overflow-y: auto;
  padding: 0 4px;
  box-sizing: border-box;
}

/* 滑动过渡 */
.slide-list-enter-active,
.slide-list-leave-active,
.slide-editor-enter-active,
.slide-editor-leave-active {
  transition: transform 0.4s ease;
}
.slide-list-enter-from,
.slide-list-leave-to {
  transform: translateX(-100%);
}
.slide-editor-enter-from {
  transform: translateX(100%);
}
.slide-editor-leave-to {
  transform: translateX(100%);
}

/* 原有样式保持不变，但需要确保滚动条正常 */
.deck-list-container {
  display: flex;
  flex-direction: column;
}
</style>