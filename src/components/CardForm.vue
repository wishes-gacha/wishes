<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { LocalizedString, Tag } from '../types';
import LocalizedInput from './common/LocalizedInput.vue';

const props = defineProps<{
  initialContent?: LocalizedString,
  initialTitle?: LocalizedString,
  initialTags?: Tag[],
  existingTags?: Tag[], // 所有已使用的标签（用于可选列表）
}>();

const emit = defineEmits<{
  (e: 'change', data: {
    content: LocalizedString,
    title: LocalizedString,
    tags: Tag[],
    valid: boolean
  }): void;
}>();

const { t } = useI18n();

// 表单数据
const localContent = ref<LocalizedString>({ ...(props.initialContent ?? {}) });
const localTitle = ref<LocalizedString>({ ...(props.initialTitle ?? {}) });
const localTags = ref<Tag[]>([...(props.initialTags || [])]);

// 新建标签输入
const newNamespace = ref('');
const newValue = ref('');

// 计算可选标签, 按命名空间分组, 排除已选的
const availableTagsGrouped = computed(() => {
  const selectedSet = new Set(localTags.value.map(t => `${t.namespace}:${t.value}`));
  const groupMap = new Map<string, Tag[]>();
  
  (props.existingTags || []).forEach(tag => {
    const key = `${tag.namespace}:${tag.value}`;
    if (selectedSet.has(key)) return; // 已选的不显示
    
    if (!groupMap.has(tag.namespace)) {
      groupMap.set(tag.namespace, []);
    }
    groupMap.get(tag.namespace)!.push(tag);
  });
  
  // 排序
  const sortedGroups = Array.from(groupMap.entries()).sort((a, b) => a[0].localeCompare(b[0]));
  return sortedGroups.map(([namespace, tags]) => ({
    namespace,
    tags: tags.sort((a, b) => a.value.localeCompare(b.value))
  }));
});

// 添加标签
function addTag() {
  const namespace = newNamespace.value.trim();
  const value = newValue.value.trim();
  if (!namespace || !value) return;
  
  // 检查是否已存在（包括已选和可选）
  const exists = localTags.value.some(t => t.namespace === namespace && t.value === value) ||
                 (props.existingTags || []).some(t => t.namespace === namespace && t.value === value);
  if (exists) {
    // 简单提示：可以选择 alert 或 toast，这里用 console
    console.warn('标签已存在');
    return;
  }
  
  localTags.value.push({ namespace, value });
  newNamespace.value = '';
  newValue.value = '';
}

// 移除标签
function removeTag(tag: Tag) {
  const index = localTags.value.findIndex(t => t.namespace === tag.namespace && t.value === tag.value);
  if (index !== -1) {
    localTags.value.splice(index, 1);
  }
}

// 添加可选标签
function addAvailableTag(tag: Tag) {
  localTags.value.push({ ...tag });
}

// 监听变化通知父组件
watch([localContent, localTitle, localTags], () => {
  emit('change', {
    content: { ...localContent.value },
    title: Object.keys(localTitle.value).length ? { ...localTitle.value } : {},
    tags: localTags.value,
    valid: Object.values(localContent.value).some(v => v.trim().length > 0),
  });
}, { deep: true, immediate: true });

const nameValid = computed(() => 
  Object.values(localContent.value).some(v => v.trim().length > 0)
)
</script>

<template>
  <div class="card-form">
    <!-- 各语言名称 -->
    <div class="form-group">
      <label>
        {{ t('card.name') }}
        <span v-if="!nameValid" class="required-hint">{{ t('card.nameRequired') }}</span>
      </label>
      <LocalizedInput v-model="localContent" :placeholder="t('card.namePlaceholder')" />
    </div>

    <!-- 各语言称号 (可选) -->
    <div class="form-group">
      <label>{{ t('card.title') }}</label>
      <LocalizedInput v-model="localTitle" :placeholder="t('card.titlePlaceholder')" />
    </div>

    <!-- 已选标签 -->
    <div class="form-group">
      <label>{{ t('card.tags') }}</label>
      <div class="selected-tags">
        <span v-for="tag in localTags" :key="`${tag.namespace}:${tag.value}`" class="tag-item">
          {{ tag.namespace }}: {{ tag.value }}
          <button class="tag-remove" @click="removeTag(tag)">✕</button>
        </span>
        <span v-if="localTags.length === 0" class="no-tags">{{ t('card.noTags') }}</span>
      </div>
    </div>

    <!-- 可选标签（按命名空间分组，横向滚动） -->
    <div class="form-group" v-if="availableTagsGrouped.length > 0">
      <label>{{ t('card.availableTags') }}</label>
      <div class="available-tags-wrapper">
        <div v-for="group in availableTagsGrouped" :key="group.namespace" class="tag-group">
          <span class="group-label">{{ group.namespace }}</span>
          <div class="group-tags">
            <span
              v-for="tag in group.tags"
              :key="`${tag.namespace}:${tag.value}`"
              class="available-tag"
              @click="addAvailableTag(tag)"
            >
              {{ tag.value }}
            </span>
          </div>
        </div>
      </div>
    </div>

    <!-- 新建标签 -->
    <div class="form-group">
      <label>{{ t('card.newTag') }}</label>
      <div class="new-tag-row">
        <input
          v-model="newNamespace"
          :placeholder="t('card.namespacePlaceholder')"
          class="new-tag-input ns"
          @keydown.enter="addTag"
        />
        <span class="sep">:</span>
        <input
          v-model="newValue"
          :placeholder="t('card.valuePlaceholder')"
          class="new-tag-input value"
          @keydown.enter="addTag"
        />
        <button class="btn-add-tag" @click="addTag">{{ t('card.addTag') }}</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.card-form {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.form-group label {
  font-size: 1.4rem;
  font-weight: 500;
  color: #b0c0d0;
}

.required-hint {
  margin-left: 5px;
  font-size: 1.2rem;
  color: #e74c3c;
}

/* 多语言输入行 */
.locale-rows {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.locale-row {
  display: flex;
  align-items: center;
  gap: 8px;
}
.locale-label {
  flex: 0 0 68px;
  font-size: 1.1rem;
  color: #6a7a90;
  text-align: right;
  user-select: none;
}

.form-input {
  flex: 1;
  background: #0b0e14;
  border: none;
  border-bottom: 3px solid #2a3340;
  /* border: 1px solid #2a3340; */
  /* border-radius: 8px; */
  padding: 6px 8px;
  color: #e4e8ef;
  font-size: 1.3rem;
  font-family: inherit;
}
.form-input:focus {
  outline: none;
  border-color: #007bff;
}

/* 已选标签 */
.selected-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  padding: 6px 0;
  user-select: none;
}
.tag-item {
  background: rgba(79, 110, 247, 0.15);
  padding: 4px 10px 4px 14px;
  border-radius: 16px;
  font-size: 1.1rem;
  color: #b0c0d0;
  display: flex;
  align-items: center;
  gap: 4px;
}
.tag-remove {
  background: none;
  border: none;
  color: #6a7a90;
  cursor: pointer;
  font-size: 0.9rem;
  padding: 0 2px;
}
.tag-remove:hover {
  color: #ff6b6b;
}
.no-tags {
  color: #4a5a70;
  font-size: 1.1rem;
}

/* 可选标签 */
.available-tags-wrapper {
  background: #0b0e14;
  /* border: 1px solid #2a3340; */
  border-radius: 8px;
  padding: 8px;
  max-height: 140px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 6px;
  user-select: none;
  scrollbar-width: thin;
  scrollbar-color: #2a3340 transparent;
}
.available-tags-wrapper::-webkit-scrollbar {
  width: 4px;
}
.available-tags-wrapper::-webkit-scrollbar-thumb {
  background: #2a3340;
  border-radius: 4px;
}

.tag-group {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.group-label {
  font-size: 1.2rem;
  font-weight: bold;
  color: #4a5a70;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}
.group-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  padding-left: 4px;
}
.available-tag {
  background: rgba(255,255,255,0.06);
  padding: 2px 12px;
  border-radius: 12px;
  font-size: 1.1rem;
  color: #b0c0d0;
  cursor: pointer;
  transition: background 0.2s;
}
.available-tag:hover {
  background: rgba(79, 110, 247, 0.2);
  color: #fff;
}

/* 新建标签 */
.new-tag-row {
  display: flex;
  align-items: center;
  gap: 6px;
  flex-wrap: wrap;
}
.new-tag-input {
  background: #0b0e14;
  border: none;
  border-bottom: 3px solid #2a3340;
  padding: 4px 10px;
  color: #e4e8ef;
  font-size: 1.2rem;
}
.new-tag-input:focus {
  outline: none;
  border-color: #007bff;
}
.new-tag-input.ns {
  flex: 1;
  min-width: 80px;
}
.new-tag-input.value {
  flex: 1;
  min-width: 80px;
}
.sep {
  color: #6a7a90;
}
.btn-add-tag {
  background: #007bff;
  border: none;
  border-radius: 6px;
  padding: 4px 16px;
  color: #fff;
  font-size: 1.1rem;
  cursor: pointer;
  white-space: nowrap;
  user-select: none;
}
.btn-add-tag:hover {
  background: #3d9bff;
}
</style>
