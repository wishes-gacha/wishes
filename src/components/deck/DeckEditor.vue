<script setup lang="ts">
import { computed, reactive, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import type { DeckSummary, EventGroup, Membership } from '../../api/decks';
import type { EventTag, Tag, LocalizedString } from '../../types';
import TagSelector from '../common/TagSelector.vue';
import ConditionEditor from '../deck/ConditionEditor.vue';
import { eventTagLabel } from '../../constants/eventTags.ts';
import EventGroupModal from './EventGroupModal.vue';
import { useDialog } from 'naive-ui';
import { cloneDeep } from 'lodash-es';
import { DEFAULT_DATA_LOCALE } from '../../composables/useLocaleText.ts';

interface DeckFormData {
  name: LocalizedString, 
  tags: Tag[],
  members: Membership,
  event_groups: Record<EventTag, EventGroup>,
}

const props = defineProps<{
  /** 传入表示编辑, 不传表示新建 */
  deck?: DeckSummary;
}>();

const emit = defineEmits<{
  (e: 'save', data: DeckFormData): void;
  (e: 'cancel'): void;
}>();

const isEditMode = computed(() => !!props.deck);

function createEmptyForm(): DeckFormData {
  return {
    name: { [DEFAULT_DATA_LOCALE]: '' },
    tags: [],
    members: { conditions: [] },
    event_groups: {},
  };
}

/** 去掉空字符串, 与后端的 LocalizedString 约定一致. */
function compactName(): LocalizedString {
  return Object.fromEntries(
    Object.entries(formData.name).filter(([, v]) => v && v.length > 0)
  );
}

/** 至少一种语言有名称才可保存 */
const hasName = computed(() => Object.keys(compactName()).length > 0)

/** 深拷贝原数据, 以便取消可回滚 */
const formData = reactive<DeckFormData>(
  props.deck
    ? {
        name: { ...props.deck.name },
        tags: cloneDeep(props.deck.tags),
        members: cloneDeep(props.deck.members),
        event_groups: cloneDeep(props.deck.event_groups),
    }
    : createEmptyForm()
);

const showEventGroupModal = ref(false);

const dialog = useDialog();
const { t } = useI18n();

function onEventGroupConfirm(tag: EventTag) {
  if (!formData.event_groups[tag]) {
    formData.event_groups[tag] = { conditions: [] };
  }
  showEventGroupModal.value = false;
}

const existingEventTags = computed(() => 
  Object.keys(formData.event_groups) as EventTag[]
)

// 添加条件到 members
function addMemberCondition() {
  formData.members.conditions.push({ type: 'tag_all', tags: [] });
}

// 删除 members 条件
function removeMemberCondition(index: number) {
  confirmDelete({
    content: t('deck.deleteMemberConfirm'),
    onConfirm: () => {
      formData.members.conditions.splice(index, 1);
    }
  })
}

// 添加 event group
function addEventGroup() {
  showEventGroupModal.value = true;
}

// 删除 event group
function removeEventGroup(key: EventTag) {
  const condCount = formData.event_groups[key]?.conditions.length ?? 0;
  confirmDelete({
    title: t('deck.deleteEventGroupTitle'),
    content: condCount > 0
      ? t('deck.deleteEventGroupWithConds', { tag: key, count: condCount })
      : t('deck.deleteEventGroup', { tag: key }),
      onConfirm: () => {
        delete formData.event_groups[key];
      }
  })
}

// 添加条件到某个 event group
function addEventGroupCondition(key: EventTag) {
  formData.event_groups[key].conditions.push({ type: 'tag_all', tags: [] });
}

// 删除 event group 中的条件
function removeEventGroupCondition(key: EventTag, index: number) {
  confirmDelete({
    content: t('deck.deleteConditionConfirm'),
    onConfirm: () => {
      formData.event_groups[key].conditions.splice(index, 1);
    }
  })
}

function confirmDelete(options: { title?: string, content: string, onConfirm: () => void }) {
  dialog.warning({
    title: options.title ?? t('deck.deleteConfirmTitle'),
    content: options.content,
    positiveText: t('deck.deleteConfirmTitle'),
    negativeText: t('common.cancel'),
    onPositiveClick: () => {
      options.onConfirm();
    },
    positiveButtonProps: {
      color: '#e74c3c',
      textColor: '#fff',
    }
  })
}

// 保存
function save() {
  if (!hasName.value) return;
  emit('save', {
    name: compactName(),
    tags: formData.tags,
    members: formData.members,
    event_groups: formData.event_groups,
  });
}
</script>

<template>
  <div class="deck-editor">
    <div class="editor-header">
      <h3>{{ isEditMode ? t('deck.editorTitleEdit') : t('deck.editorTitleCreate') }}</h3>
    </div>

    <div class="id-name-field">
      <label class="deck-name-title">{{ t('deck.nameLabel') }}</label>
      <LocalizedInput
        v-model="formData.name"
        :placeholder="t('deck.namePlaceholder')"
      />
    </div>

    <div class="editor-body">

      <!-- 标签 -->
      <div class="field">
        <label>{{ t('deck.tagsLabel') }}</label>
        <TagSelector v-model="formData.tags" />
      </div>

      <!-- Membership -->
      <div class="field">
        <label>{{ t('deck.membersLabel') }}</label>
        <span>{{ t('deck.membersHint') }}</span>

        <div v-for="(_, idx) in formData.members.conditions" :key="idx" class="condition-wrapper">
          <ConditionEditor v-model="formData.members.conditions[idx]" />
          <button class="btn-remove-cond" @click="removeMemberCondition(idx)">{{ t('deck.deleteCondition') }}</button>
        </div>

        <button class="btn-add-cond" @click="addMemberCondition">{{ t('deck.addCondition') }}</button>
      </div>

      <!-- Event Groups -->
      <div class="field">
        <label>{{ t('deck.eventGroupsLabel') }}</label>
        <span>{{ t('deck.eventGroupsHint') }}</span>

        <div v-for="(group, eventTag) in formData.event_groups" :key="eventTag" class="event-group">
          <div class="event-group-header">
            <div class="event-tag">
              {{ eventTag }}
              <span v-if="eventTagLabel(t, eventTag)" class="event-tag-hint">
                 {{ eventTagLabel(t, eventTag) }}
              </span>
            </div>
            <button class="btn-remove-group" @click="removeEventGroup(eventTag)">{{ t('deck.removeGroup') }}</button>
          </div>

          <div v-for="(_, idx) in group.conditions" :key="idx" class="condition-wrapper">
            <ConditionEditor v-model="group.conditions[idx]" />
            <button class="btn-remove-cond" @click="removeEventGroupCondition(eventTag, idx)">{{ t('deck.deleteCondition') }}</button>
          </div>

          <button class="btn-add-cond" @click="addEventGroupCondition(eventTag)">{{ t('deck.addCondition') }}</button>
        </div>

        <button class="btn-add-group" @click="addEventGroup">{{ t('deck.addGroup') }}</button>

        <EventGroupModal
          v-if="showEventGroupModal"
          :existing-tags="existingEventTags"
          @confirm="onEventGroupConfirm"
          @close="showEventGroupModal = false"
        />
      </div>
    </div>

    <div class="editor-footer">
      <button class="btn-cancel" @click="emit('cancel')">{{ t('common.cancel') }}</button>
      <button class="btn-save" @click="save" :disabled="!hasName">{{ t('deck.save') }}</button>
    </div>
  </div>
</template>

<style scoped>
.deck-editor {
  display: flex;
  flex-direction: column;
  height: 100%;
  /* padding: 16px; */
  user-select: none;
}

.editor-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 16px;
  flex-shrink: 0;
}
.editor-header h3 {
  font-size: 2rem;
  margin: 0;
}
.btn-toggle-locales {
  flex: 0 0 auto;
  position: relative;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: #212830;
  border: 1px solid #2d3642;
  border-radius: 4px;
  padding: 6px 10px;
  color: #8a99b4;
  font-size: 1.5rem;
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}
.btn-toggle-locales:hover {
  background: #2d3642;
  color: #e0e3e7;
}
.btn-toggle-locales .toggle-arrow {
  display: inline-block;
  transition: transform 0.5s ease;
}
.btn-toggle-locales.active .toggle-arrow {
  transform: rotate(180deg);
}
.locale-badge {
  background: #007bff;
  color: #fff;
  border-radius: 8px;
  padding: 0 6px;
  font-size: 1rem;
  line-height: 16px;
}

.other-locales {
  display: flex;
  flex-direction: column;
  gap: 4px;
  overflow: hidden;
}

/* 展开 / 折叠动画 */
.locale-slide-enter-active,
.locale-slide-leave-active {
  transition: max-height 0.5s ease, opacity 0.3s ease;
}
.locale-slide-enter-from,
.locale-slide-leave-to {
  max-height: 0;
  opacity: 0;
}
.locale-slide-enter-to,
.locale-slide-leave-from {
  max-height: 600px; /* 足够容纳很多语言 */
  opacity: 1;
}

.editor-body {
  flex: 1;
  overflow-y: auto;
  padding-right: 8px;
  scrollbar-width: thin;
  scrollbar-color: #2a3340 transparent;
}
.editor-body::-webkit-scrollbar {
  width: 4px;
}
.editor-body::-webkit-scrollbar-thumb {
  background: #2a3340;
  border-radius: 4px;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  border-top: 1px solid #2a3340;
  padding-top: 10px;
  margin-bottom: 20px;
}
.field label {
  font-size: 1.7rem;
  font-weight: bold;
  color: #b0c0d0;
  font-weight: bold;
}
.field span {
  font-size: 1.3rem;
  color: #8a99b4;
}

.id-name-field {
  display: flex;
  flex-direction: row;
  gap: 6px;
  align-items: center;
  margin-bottom: 12px;
}

.deck-name-title {
  flex-shrink: 0;
  font-size: 1.5rem;
  color: #b0c0d0;
}

.name-locales {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-bottom: 12px;
}
.name-locale-row {
  display: flex;
  align-items: center;
  gap: 8px;
}
.name-locale-label {
  flex: 0 0 68px;
  font-size: 1.1rem;
  color: #6a7a90;
  text-align: right;
  user-select: none;
}

.form-input {
  width: 100%;
  background: #0b0e14;
  border: none;
  border-bottom: 3px solid #2a3340;
  padding: 6px 8px;
  color: #e4e8ef;
  font-size: 1.3rem;
  font-family: inherit;
}
.form-input:focus {
  outline: none;
  border-color: #007bff;
}

.event-tag-hint {
  color: #6a7a90;
  font-weight: normal;
  font-size: 1.2rem;
  margin-left: 4px;
}

.condition-wrapper {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 8px;
  background: rgba(26, 33, 43, 0.3);
  padding: 6px 10px;
  border-radius: 6px;
}
.condition-wrapper > * {
  flex: 1;
}

.btn-remove-cond {
  flex: 0 0 auto;
  background: #212830;
  border: 1px solid #3d444d;
  border-radius: 4px;
  padding: 4px 14px;
  color: #fa5e55;
  cursor: pointer;
  font-size: 1.2rem;
  font-weight: bold;
}
.btn-remove-cond:hover {
  background: #b62324;
  color: #ffe9ca;
}

.btn-add-cond,
.btn-add-group {
  background: #3b4b5e;
  border: none;
  border-radius: 6px;
  padding: 4px 16px;
  color: #fff;
  font-size: 1.1rem;
  cursor: pointer;
  margin-top: 4px;
}
.btn-add-cond:hover,
.btn-add-group:hover {
  background: #475a70;
}

.event-group {
  border-left: 3px solid #2a3340;
  padding-left: 12px;
  margin-bottom: 12px;
}
.event-group-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}
.event-tag {
  font-size: 1.4rem;
  font-weight: bold;
  color: #4f6ef7;
}
.btn-remove-group {
  background: #212830;
  border: 1px solid #3d444d;
  border-radius: 4px;
  padding: 4px 14px;
  color: #fa5e55;
  cursor: pointer;
  font-size: 1.2rem;
  font-weight: bold;
}
.btn-remove-group:hover {
  background: #b62324;
  color: #ffe9ca;
}

.editor-footer {
  display: flex;
  gap: 12px;
  justify-content: flex-end;
  padding-top: 16px;
  /* border-top: 1px solid #2a3340; */
  flex-shrink: 0;
}
.btn-save {
  background: #007bff;
  border: none;
  border-radius: 8px;
  padding: 8px 24px;
  color: #fff;
  font-weight: 600;
  cursor: pointer;
}
.btn-save:hover:not(:disabled) {
  background: #3d9bff;
}
.btn-save:disabled {
  opacity: 0.4;
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