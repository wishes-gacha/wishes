<script setup lang="ts">
import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import type { EditableCondition } from '../../api/decks';
import TagSelector from '../common/TagSelector.vue';
import CardSelectorModal from './CardSelectorModal.vue';
import { NSelect, SelectOption } from 'naive-ui';


const props = defineProps<{
  modelValue: EditableCondition;
}>();

const emit = defineEmits<{
  (e: 'update:modelValue', value: EditableCondition): void;
}>();

const condition = computed({
  get: () => props.modelValue,
  set: (val) => emit('update:modelValue', val),
});

const { t } = useI18n();

const typeOptions = computed<SelectOption[]>(() => [
  { label: t('condition.all'),           value: 'all' },
  { label: t('condition.tagAll'),        value: 'tag_all' },
  { label: t('condition.tagAny'),        value: 'tag_any' },
  { label: t('condition.filterTagAll'),  value: 'filter_tag_all' },
  { label: t('condition.filterTagAny'),  value: 'filter_tag_any' },
  { label: t('condition.includeCards'),  value: 'include_ids' },
  { label: t('condition.excludeCards'),  value: 'exclude_ids' },
]);

// 条件类型收窄
const isTagType = computed(() => 
  ['tag_all', 'tag_any', 'filter_tag_all', 'filter_tag_any'].includes(condition.value.type)
);
const isIdType = computed(() => 
  ['include_ids', 'exclude_ids'].includes(condition.value.type)
);

const currentIds = computed<string[]>(() => 
  isIdType.value ? (condition.value as {ids: string[]}).ids ?? [] : []
)

const selectedType = computed({
  get: () => condition.value.type,
  set: (type: string) => changeType(type),
});

function changeType(type: string) {
  if (condition.value.type === type) return;

  if (['tag_all', 'tag_any', 'filter_tag_all', 'filter_tag_any'].includes(type)) {
    condition.value = { type, tags: [] } as EditableCondition;
  } else if (['include_ids', 'exclude_ids'].includes(type)) {
    // 卡片引用使用目标 Card 的 global_id
    condition.value = { type, ids: [] } as EditableCondition;
  } else if (type === 'all') {
    condition.value = { type: 'all' } as EditableCondition;
  }
}

// 标签更新
function updateTags(newTags: any[]) {
  condition.value = { ...condition.value, tags: newTags } as EditableCondition;
}

const showCardSelector = ref(false);

// 打开卡片选择器
function openCardSelector() {
  if (!isIdType.value) return;
  showCardSelector.value = true;
}

function onCardConfirm(ids: string[]) {
  condition.value = { ...condition.value, ids } as EditableCondition;
  showCardSelector.value = false;
}
</script>

<template>
  <div class="condition-editor">
    <n-select
      v-model:value="selectedType"
      :options="typeOptions"
      size="small"
      class="cond-type-select"
      :consistent-menu-width="false"
    />

    <!-- 标签选择 -->
    <template v-if="isTagType">
      <TagSelector
        :model-value="(condition as any).tags"
        @update:model-value="updateTags"
      />
    </template>

    <!-- ID 选择 -->
    <template v-if="isIdType">
      <div class="id-selector">
        <span class="id-count">{{ t('condition.selectedCards', { count: currentIds.length }) }}</span>
        <button class="btn-select-cards" @click="openCardSelector">{{ t('condition.selectCards') }}</button>
      </div>
    </template>

    <CardSelectorModal
      v-if="showCardSelector"
      :initial-selected-ids="currentIds"
      @confirm="onCardConfirm"
      @close="showCardSelector = false"
    />
  </div>
</template>

<style scoped>
.condition-editor {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  width: 100%;
}

.cond-type-select {
  flex: 0 0 auto;
  min-width: 130px;
  width: auto;
}

.id-selector {
  display: flex;
  align-items: center;
  gap: 8px;
  background: #0b0e14;
  padding: 0 0 0 12px;
  border-radius: 4px;
}
.id-count {
  color: #8a99b4;
  font-size: 1.1rem;
}
.btn-select-cards {
  background: #3b4b5e;
  border: none;
  border-radius: 4px;
  padding: 2px 12px;
  color: #fff;
  cursor: pointer;
  font-size: 1.2rem;
}
.btn-select-cards:hover {
  background: #475a70;
}
</style>