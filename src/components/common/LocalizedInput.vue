<!-- components/LocalizedInput.vue -->
<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { NButton, NEmpty, NInput, NPopover, useThemeVars } from 'naive-ui'
import { useI18n } from 'vue-i18n'
import type { LocalizedString } from '../../types';

defineOptions({ name: 'LocalizedInput' })

const props = withDefaults(
  defineProps<{
    /** 绑定的多语言字符串 */
    modelValue: LocalizedString
    /** 可选语言列表, 缺省取 vue-i18n 的 availableLocales */
    languages?: string[]
    /** 主语言, 缺省取当前 i18n locale */
    mainLanguage?: string
    /** 主语言输入框占位符, 缺省走 i18n 文案 */
    placeholder?: string
    disabled?: boolean
    clearable?: boolean
    size?: 'tiny' | 'small' | 'medium' | 'large'
    /** 浮窗宽度 (px) */
    panelWidth?: number
    /** 浮窗内容区最大高度 (px), 超出滚动 */
    maxPanelHeight?: number
    /** 是否从输出中剔除空字符串 (与后端「不存储空字符串」约定一致) */
    omitEmpty?: boolean
  }>(),
  {
    languages: undefined,
    mainLanguage: undefined,
    placeholder: undefined,
    disabled: false,
    clearable: true,
    size: 'medium',
    panelWidth: 320,
    maxPanelHeight: 280,
    omitEmpty: true,
  },
)

const emit = defineEmits<{
  (e: 'update:modelValue', value: LocalizedString): void
}>()

const { locale, availableLocales, t } = useI18n()
const themeVars = useThemeVars()

/* ------------------------------------------------------------------ *
 * 语言集合
 * ------------------------------------------------------------------ */

const mainLang = computed(() => props.mainLanguage || locale.value)

/** 所有需要参与编辑的语言: 配置语言 ∪ 主语言 ∪ 数据中已存在的 key */
const languagePool = computed<string[]>(() => {
  const set = new Set<string>()
  const configured = props.languages?.length ? props.languages : availableLocales
  for (const lang of configured) set.add(lang)
  set.add(mainLang.value)
  // 保留数据里已有但不在语言列表中的 key, 避免用户数据「不可见」
  for (const key of Object.keys(props.modelValue ?? {})) set.add(key)
  return Array.from(set)
})

const otherLanguages = computed(() =>
  languagePool.value.filter((lang) => lang !== mainLang.value),
)

/* ------------------------------------------------------------------ *
 * 内部状态与双向绑定
 * ------------------------------------------------------------------ */

const inner = ref<LocalizedString>({})

/** 剔除空字符串, 返回新对象 */
function omitEmptyValues(source: LocalizedString): LocalizedString {
  const result: LocalizedString = {}
  for (const [key, value] of Object.entries(source ?? {})) {
    if (typeof value === 'string' && value !== '') result[key] = value
  }
  return result
}

function isSameMap(a: LocalizedString, b: LocalizedString): boolean {
  const keysA = Object.keys(a)
  if (keysA.length !== Object.keys(b).length) return false
  return keysA.every((key) => a[key] === b[key])
}

watch(
  () => props.modelValue,
  (value) => {
    // 归一化后一致 → 说明这次变更源自本组件自身, 跳过回写以免打断输入
    if (isSameMap(omitEmptyValues(value ?? {}), omitEmptyValues(inner.value))) return
    inner.value = { ...(value ?? {}) }
  },
  { deep: true, immediate: true },
)

function setLang(lang: string, value: string) {
  if (inner.value[lang] === value) return
  inner.value[lang] = value
  emit(
    'update:modelValue',
    props.omitEmpty ? omitEmptyValues(inner.value) : { ...inner.value },
  )
}

/* ------------------------------------------------------------------ *
 * 展示辅助
 * ------------------------------------------------------------------ */

/** 用 Intl.DisplayNames 把语言标签本地化, 例如 zh-CN → 中文 (中国) */
const displayNames = computed<Intl.DisplayNames | null>(() => {
  try {
    return new Intl.DisplayNames([locale.value], { type: 'language' })
  } catch {
    return null
  }
})

function languageLabel(lang: string): string {
  try {
    return displayNames.value?.of(lang) || lang
  } catch {
    return lang
  }
}

const hasOthers = computed(() =>
  otherLanguages.value.some((lang) => (inner.value[lang] ?? '') !== ''),
)
</script>

<template>
  <div class="localized-input">
    <!-- 主语言: 常驻显示 -->
    <NInput
      class="localized-input__main"
      :value="inner[mainLang] ?? ''"
      :placeholder="props.placeholder"
      :disabled="disabled"
      :clearable="clearable"
      :size="size"
      @update:value="(value: string) => setLang(mainLang, value)"
    />

    <!-- 下拉按钮 + 其他语言浮窗 -->
    <NPopover
      trigger="click"
      placement="bottom-end"
      :disabled="disabled"
      :style="{ padding: '12px 14px' }"
    >
      <template #trigger>
        <div class="localized-input__trigger">
          <NButton
            :size="size"
            :disabled="disabled"
            :type="hasOthers ? 'primary' : 'default'"
            secondary
          >
            <template #icon>
              <svg
                class="localized-input__icon"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="1.8"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
              >
                <circle cx="12" cy="12" r="9" />
                <path d="M3 12h18" />
                <path d="M12 3c2.6 3 2.6 15 0 18" />
                <path d="M12 3c-2.6 3-2.6 15 0 18" />
              </svg>
            </template>
          </NButton>
          <span
            v-if="hasOthers"
            class="localized-input__dot"
            :style="{
              background: themeVars.primaryColor,
              borderColor: themeVars.bodyColor,
            }"
          />
        </div>
      </template>

      <div class="localized-input__panel" :style="{ width: `${panelWidth}px` }">
        <div
          class="localized-input__panel-title"
          :style="{ color: themeVars.textColor2 }"
        >
          {{ t('localizedInput.panelTitle') }}
        </div>

        <div
          v-if="otherLanguages.length"
          class="localized-input__list"
          :style="{ maxHeight: `${maxPanelHeight}px` }"
        >
          <div
            v-for="lang in otherLanguages"
            :key="lang"
            class="localized-input__row"
          >
            <span
              class="localized-input__lang"
              :style="{ color: themeVars.textColor3 }"
              :title="lang"
            >
              {{ languageLabel(lang) }}
            </span>
            <NInput
              :value="inner[lang] ?? ''"
              size="small"
              :disabled="disabled"
              :placeholder="props.placeholder"
              @update:value="(value: string) => setLang(lang, value)"
            />
          </div>
        </div>

        <NEmpty
          v-else
          size="small"
          :description="t('localizedInput.noOtherLanguages')"
        />
      </div>
    </NPopover>
  </div>
</template>

<style scoped>
.localized-input {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
}

.localized-input__main {
  flex: 1 1 auto;
  min-width: 0;
}

.localized-input__trigger {
  position: relative;
  display: inline-flex;
  flex: 0 0 auto;
}

.localized-input__icon {
  display: block;
  width: 16px;
  height: 16px;
}

.localized-input__dot {
  position: absolute;
  top: -3px;
  right: -3px;
  box-sizing: content-box;
  width: 6px;
  height: 6px;
  border: 2px solid transparent;
  border-radius: 50%;
  pointer-events: none;
}

.localized-input__panel {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.localized-input__panel-title {
  font-size: 12px;
  font-weight: 600;
  line-height: 1.4;
}

.localized-input__list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  overflow-y: auto;
  padding-right: 2px;
}

.localized-input__row {
  display: grid;
  grid-template-columns: 96px minmax(0, 1fr);
  align-items: center;
  gap: 8px;
}

.localized-input__lang {
  overflow: hidden;
  font-size: 12px;
  line-height: 1.4;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>