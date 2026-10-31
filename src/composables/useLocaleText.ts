import { useI18n } from "vue-i18n";
import { i18n } from "../i18n";
import { LocalizedString } from "../types";
import { computed, MaybeRefOrGetter, toValue } from "vue";

/**
 * 数据层多语言文本的回退顺序: 当前语言 -> 默认语言 -> 任意可用语言.
 *
 * 与后端 `LocalizedString::get_with_fallback` 的顺序保持一致,
 * 后端不做回退, 由前端按当前界面语言解析.
 */
export const DEFAULT_DATA_LOCALE = "zh-CN";

/** 解析多语言文本, 无任何可用翻译时返回空字符串. */
export function pickLocalized(text?: LocalizedString, locale?: string): string {
  if (!text) return '';

  const current = locale ?? String(i18n.global.locale.value);
  return text[current]
    ?? text[DEFAULT_DATA_LOCALE]
    ?? Object.values(text)[0]
    ?? '';
}

/** 多语言文本的全部非空翻译, 用于跨语言搜索 */
export function allLocalizedValues(text?: LocalizedString): string[] {
  if (!text) return [];
  return Object.values(text).filter(v => v.length > 0);
}

/**
 * 展示用短标识: Uuid 的后 8 位.
 * 正常情况不应使用本函数, 本函数仅用于部分情况下的回退显示.
*/
export function shortGlobalId(globalId?: string): string {
  if (!globalId) return '';
  return globalId.replace(/-/g, '').slice(-8);
}

/** 对象来源本地化展示 */
export function originLabel(t: (k: string, p?: any) => string, origin: string): string {
  if (origin.startsWith('pack:')) {
    return t('origin.pack', { name: origin.slice(5) });
  }
  return t(`origin.${origin}`);   // official / local
}

/**
 * 在组件中解析多语言文本的组合式函数.
 *
 * ```ts
 * const { pick, currentLocale } = useLocaleText();
 * const title = computed(() => pick(card.content.value));
 * ```
 */
export function useLocaleText() {
  const { locale } = useI18n();

  /** 当前界面语言 */
  const currentLocale = computed(() => String(locale.value));

  /**
   * 解析一段多语言文本.
   *
   * 参数可以是 `LocalizedString`、`ref` 或 `getter`, 因此可直接在模板中调用.
   */
  function pick(text?: MaybeRefOrGetter<LocalizedString | undefined>): string {
    return pickLocalized(toValue(text), currentLocale.value);
  }

  return {
    pick,
    pickLocalized,
    currentLocale,
    shortGlobalId,
    originLabel
  };
}

/** 非组件环境下的展示用短标识 (等价于 `shortGlobalId`). */
export const formatGlobalId = shortGlobalId;
