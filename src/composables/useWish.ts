/**
 * 抽卡相关的组合式函数, 封装状态和后端 API 调用
 * 提供:
 * - 卡池列表加载
 * - 卡池详情加载
 * - 执行单抽/十连 (十连由调用方循环)
 * - 历史记录、加载状态、错误信息
 *
 * 数据格式 v2 起, 卡池标识为 `global_id` (Uuid 字符串).
 */

import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { BannerInfo, BannerSummary, WishResponse } from "../types";
import { invoke } from "@tauri-apps/api/core";


export function useWish() {
  const banners = ref<BannerSummary[]>([]);
  const currentBannerInfo = ref<BannerInfo | null>(null);
  const currentWish = ref<WishResponse | null>(null);
  const history = ref<WishResponse[]>([]);
  const loading = ref(false);
  const error = ref<string | null>(null);

  const { t } = useI18n();

  /** 加载卡池摘要列表 (被派生版本遮蔽的卡池不会出现) */
  async function loadBanners() {
    try {
      banners.value = await invoke<BannerSummary[]>("get_banners");
    } catch (e) {
      error.value = t("wish.loadBannersFailed", { error: e });
    }
  }

  /** 加载卡池详细信息 */
  async function loadBannerInfo(bannerId: string) {
    try {
      currentBannerInfo.value = await invoke<BannerInfo>("get_banner_info", { bannerId });
    } catch (e) {
      error.value = t("wish.loadBannerInfoFailed", { error: e });
    }
  }

  /** 执行一次抽卡, 并更新相关状态和历史记录 */
  async function doWish(bannerId: string) {
    loading.value = true;
    error.value = null;
    try {
      const result = await invoke<WishResponse>("wish", { bannerId });
      currentWish.value = result;
      history.value.unshift(currentWish.value);
      await loadBannerInfo(bannerId);
      return result;
    } catch (e) {
      error.value = t("wish.drawFailed", { error: e })
      throw e;
    } finally {
      loading.value = false;
    }
  }

  /** 执行十连抽卡 (后端批量), 并更新相关状态和历史记录 */
  async function doWishTen(bannerId: string): Promise<WishResponse[]> {
    loading.value = true;
    error.value = null;
    try {
      const results = await invoke<WishResponse[]>("wish_ten", { bannerId });
      history.value.unshift(...results);
      await loadBannerInfo(bannerId);
      return results;
    } catch (e) {
      error.value = t("wish.drawFailed", { error: e });
      throw e;
    } finally {
      loading.value = false;
    }
  }

  return {
    banners,
    currentBannerInfo,
    currentWish,
    history,
    loading,
    error,
    loadBanners,
    loadBannerInfo,
    doWish,
    doWishTen,
  }
}
