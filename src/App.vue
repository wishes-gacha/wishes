<script setup lang="ts">
/**
 * 应用根组件
 * - 管理页面导航和底部导航
 * - 加载卡池列表, 默认选中第一个
 * - 处理抽卡逻辑, 并展示结果浮层
 * - 卡池标签栏的滚动定位
 */

import { computed, defineAsyncComponent, nextTick, ref, watch } from "vue";
import BottomNav from "./components/BottomNav.vue";
import { useWish } from "./composables/useWish";
import { pickLocalized } from "./composables/useLocaleText.ts";
import type { WishResponse } from "./types.ts";
import WishResultOverlay from "./components/WishResultOverlay.vue";
import { GlobalThemeOverrides, NConfigProvider, darkTheme, NDialogProvider, NMessageProvider, createDiscreteApi } from "naive-ui";

const {
  banners,
  currentBannerInfo,
  currentWish,
  error,
  loadBanners,
  loadBannerInfo,
  doWish,
  doWishTen,
} = useWish();

const selectedBannerId = ref<string | null>(null);

// 初始化加载
loadBanners();

// 默认选中第一个卡池, 并加载信息
watch(banners, (new_banners) => {
  if (new_banners.length > 0 && selectedBannerId.value === null) {
    selectedBannerId.value = new_banners[0].global_id;
    loadBannerInfo(selectedBannerId.value);
  }
  scrollToActiveBannerTab();
});

function onSelectBanner(bannerGlobalId: string) {
  selectedBannerId.value = bannerGlobalId;
  loadBannerInfo(bannerGlobalId);
  navigateTo("gacha");
}

async function onBannerChanged(bannerGlobalId: string) {
  selectedBannerId.value = bannerGlobalId;
  await loadBannerInfo(selectedBannerId.value)
  scrollToActiveBannerTab();
}

function scrollToActiveBannerTab() {
  // 未按预期工作
  nextTick(() => {
    const container = document.querySelector('.banner-tabs') as HTMLElement;
    const activeTab = container?.querySelector('.banner-tab.active') as HTMLElement;
    if (container && activeTab) {
      const containerWidth = container.offsetWidth;
      const tabWidth = activeTab.offsetWidth;
      const tabLeft = activeTab.offsetLeft;
      const scrollTarget = tabLeft - containerWidth / 2 + tabWidth / 2;
      container.scrollTo({ left: scrollTarget, behavior: 'smooth' });
    }
  });
}


// 抽卡相关状态
const showWishResultOverlay = ref(false);
const tenWishResults = ref<WishResponse[]>([]);
const isTenWish = ref(false);

/** 单抽处理 */
async function handleWishSingle() {
  if (!selectedBannerId.value) return;
  try {
    const response = await doWish(selectedBannerId.value);
    tenWishResults.value = [response];
    isTenWish.value = false;
    showWishResultOverlay.value = true;
  } catch (e) {
    discreteMessage.error(error.value ?? String(e));
  }
}

/** 十连处理 (后端批量执行) */
async function handleWishTen() {
  if (!selectedBannerId.value) return;
  try {
    tenWishResults.value = await doWishTen(selectedBannerId.value);
    isTenWish.value = true;
    showWishResultOverlay.value = true;
  } catch (e) {
    discreteMessage.error(error.value ?? String(e));
  }
}

function closeResultOverlay() {
  showWishResultOverlay.value = false;
  currentWish.value = null;
  isTenWish.value = false;
  tenWishResults.value = [];
}

// 页面懒加载
const HomePage = defineAsyncComponent(() => import("./views/HomePage.vue"));
const GachaPage = defineAsyncComponent(() => import("./views/GachaPage.vue"));
const CatalogPage = defineAsyncComponent(() => import("./views/CatalogPage.vue"));
const SettingsPage = defineAsyncComponent(() => import("./views/SettingsPage.vue"));

const currentPage = ref("home");
const isGachaPage = computed(() => currentPage.value === "gacha");

const pageMap: Record<string, any> = {
  home: HomePage,
  gacha: GachaPage,
  catalog: CatalogPage,
  settings: SettingsPage,
};

const currentPageComponent = computed(() => pageMap[currentPage.value]);

/** 切换页面 */
function navigateTo(page: string) {
  currentPage.value = page;
}

const themeOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: '#007bff',
    primaryColorHover: '#3d9bff',
    primaryColorPressed: '#0066d6',
    borderRadius: '6px',
    bodyColor: '#121316',
    cardColor: '#1a212b',
    borderColor: '#2a3340',
    textColorBase: '#e4e8ef',
    textColor2: '#b0c0d0',
    textColor3: '#6a7a90',
    fontFamily: "'Inter', system-ui, -apple-system, sans-serif",
  },
  Select: {
    peers: {
      InternalSelection: {
        color: 'rgba(11, 14, 20, 0.6)',
        colorActive: 'rgba(11, 14, 20, 0.6)',
        border: '1px solid #2a3340',
        borderActive: '1px solid #007bff',
        borderHover: '1px solid #4f6ef7',
        borderFocus: '1px solid #007bff',
        textColor: '#b0c0d0',
        // textColorActive: '#fff',
      },
      InternalSelectMenu: {
        color: '#1a212b',
        optionTextColor: '#e4e8ef',
        optionTextColorActive: '#fff',
        optionColorPending: 'rgba(79, 110, 247, 0.15)',
        optionColorActive: 'rgba(79, 110, 247, 0.25)',
      },
    },
  },
};

/** 独立提示 API (App 根组件不在 NMessageProvider 上下文内) */
const { message: discreteMessage } = createDiscreteApi(['message'], {
  configProviderProps: { theme: darkTheme, themeOverrides },
});
</script>


<template>
  <n-config-provider :theme="darkTheme" :theme-overrides="themeOverrides">
    <n-dialog-provider>
      <n-message-provider>
        <div id="app">
          <div class="app-title">Wishes 众愿</div>
          <header :class="{ expanded:  isGachaPage }">
            <div class="header-content">
              <div v-if="isGachaPage" class="banner-tabs">
                <div
                  v-for="b in banners"
                  :key="b.global_id"
                  class="banner-tab"
                  :class="{ active: selectedBannerId === b.global_id }"
                  @click="onBannerChanged(b.global_id)"
                >
                  {{ pickLocalized(b.name) }}
                </div>
              </div>
            </div>
          </header>

          <div class="page-container">
            <Transition name="slide" mode="out-in">
              <component
                :is="currentPageComponent"
                :key="currentPage"
                @select-banner="onSelectBanner"
                :banner-id="selectedBannerId"
                :banner-info="currentBannerInfo"
                :banners="banners"
                @wish-single="handleWishSingle"
                @wish-ten="handleWishTen"
              />
            </Transition>
          </div>

          <BottomNav v-model="currentPage"/>

          <WishResultOverlay
            v-if="showWishResultOverlay"
            :result="currentWish"
            :is-ten-wish="isTenWish"
            :ten-results="tenWishResults"
            @close="closeResultOverlay"
          />
        </div>
      </n-message-provider>
    </n-dialog-provider>
  </n-config-provider>
</template>


<style scoped>
.app-title {
  position: fixed;
  top: 0;
  left: 0;
  padding: 1rem 1.2rem;
  font-size: 2rem;
  letter-spacing: 0.5px;
  color: #e4e8ef;
  pointer-events: none;
  z-index: 20;
}

header {
  background: transparent;
  height: 0;
  transition: height 0.4s cubic-bezier(0.34, 1.56, 0.64, 1),
              background-color 0.4s ease;
  overflow: hidden;
  display: flex;
  align-items: flex-end;
  padding-top: 1rem;
  padding-left: 1rem;
  flex-shrink: 0;
  z-index: 10;
  user-select: none;
}

header.expanded {
  background-color: #1c1e22;
  height: 8rem;
  align-items: flex-end;
  padding-top: 1rem;
}

.header-content {
  display: flex;
  flex-direction: column;
  width: 100%;
  gap: 0.2rem;
}

.banner-tabs {
  display: flex;
  gap: 0.8rem;
  overflow-x: auto;
  margin: 1rem;
  padding: 0.2rem;
  scrollbar-width: none;
  justify-content: center;
  scroll-behavior: smooth;
}

.banner-tabs::-webkit-scrollbar {
  display: none;
}

.banner-tab {
  gap: 2px;
  padding: 5px 40px;
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.06);
  color: #f4f4f4;
  font-size: 1.5rem;
  white-space: nowrap;
  cursor: pointer;
  transition: all 0.25s;
  flex-shrink: 0;
  font-weight: 500;
}

.banner-tab:hover {
  background: rgba(255, 255, 255, 0.12);
}

.banner-tab.active {
  background: #007bff;
  color: #fff;
  box-shadow: 0 4px 4px rgba(79, 110, 247, 0.3);
}

.page-container {
  flex: 1;
  padding-bottom: 5rem;   /* 底栏高度为 5rem */
  overflow: hidden;
}

.slide-enter-active {
  transition: all 0.6s cubic-bezier(0.25, 0.10, 0.25, 1.00);
}

.slide-leave-active {
  transition: all 0s;
}

.slide-enter-from {
  transform: translateY(100%);
  opacity: 0;
}
</style>

<style>
html {
  font-size: 62.5%;   /* 使 1rem == 10px */
  overflow: hidden;
  user-select: none;
}

* {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

#app {
  background: #121316;
  height: 100vh;          /* 填充视图 */
  overflow: hidden;       /* 隐藏滚动条 */
  display: flex;
  flex-direction: column;
}

body {
  font-family: 'Inter', system-ui, -apple-system, sans-serif;
  background: #0b0e14;
  color: #e4e8ef;
  margin: 0;
  padding: 0;
  min-height: 100%;
  overflow: hidden;
}
</style>