<script setup lang="ts">
/**
 * 图鉴页面, 管理卡片、卡组、逻辑与卡池
 */

import { computed, ref } from 'vue';
import { useI18n } from 'vue-i18n';
import CardManager from '../components/catalog/CardManager.vue';
import DeckManager from '../components/catalog/DeckManager.vue';
import LogicManager from '../components/catalog/LogicManager.vue';
import BannerManager from '../components/catalog/BannerManager.vue';


const { t } = useI18n();

const tabs = [
  { key: "cards", labelKey: "catalog.tabs.cards" },
  { key: "decks", labelKey: "catalog.tabs.decks" },
  { key: "logics", labelKey: "catalog.tabs.logics" },
  { key: "banners", labelKey: "catalog.tabs.banners" },
  // 后续补充
];
const activeTab = ref("cards");

const currentComponent = computed(() => {
  switch (activeTab.value) {
    case "cards": return CardManager;
    case "decks": return DeckManager;
    case "logics": return LogicManager;
    case "banners": return BannerManager;
    default: return CardManager;
  }
})
</script>


<template>
  <div class="page catalog-page">
    <div class="catalog-header">
      <span class="title">{{ t('catalog.title') }}</span>
      <span class="subtitle">{{ t('catalog.subtitle') }}</span>
    </div>
    
    <!-- 标签栏 -->
    <div class="tab-bar">
      <button
        v-for="tab in tabs"
        :key="tab.key"
        class="tab-btn"
        :class="{ active: activeTab === tab.key }"
        @click="activeTab = tab.key"
      >
        {{ t(tab.labelKey) }}
      </button>
    </div>

    <!-- 内容区 -->
     <div class="tab-content">
      <component :is="currentComponent" />
     </div>
    
    <!-- <div class="catalog-grid">
      <div class="catalog-card">
        <div class="catalog-icon">🃏</div>
        <div class="catalog-title">卡片</div>
        <div class="catalog-count">{{ stats ? `${stats.cards} 张` : "加载失败" }}</div>
      </div>
      <div class="catalog-card">
        <div class="catalog-icon">📋</div>
        <div class="catalog-title">卡组</div>
        <div class="catalog-count">{{ stats ? `${stats.decks} 个` : "无法加载" }}</div>
      </div>
      <div class="catalog-card">
        <div class="catalog-icon">⚙️</div>
        <div class="catalog-title">抽卡逻辑</div>
        <div class="catalog-count">{{ stats ? `${stats.logics} 个` : "无法加载" }}</div>
      </div>
    </div> -->
  </div>
</template>


<style scoped>
.catalog-page {
  padding: 40px 16px 20px;
  height: 100%;
  background-image: radial-gradient(
    ellipse at 10% 90%,
    rgba(77, 109, 255, 0.1) 0%,
    rgba(120, 80, 240, 0.06) 40%,
    transparent 70%
  );
  overflow-y: hidden;
  display: flex;
  flex-direction: column;
}

.catalog-header {
  margin-bottom: 16px;
  flex-shrink: 0;
}

.title {
  font-size: 2.5rem;
  font-weight: 600;
  user-select: none;
}

.subtitle {
  color: #8a99b4;
  font-size: 1.2rem;
  padding: 10px;
  user-select: none;
}

.tab-bar {
  flex-shrink: 0;
  display: flex;
  gap: 8px;
  padding-bottom: 8px;
  flex-wrap: wrap;
}

.tab-btn {
  background: transparent;
  border: none;
  padding: 8px 20px;
  border-radius: 8px 8px 0 0;
  font-size: 1.3rem;
  font-weight: 500;
  color: #8a99b4;
  cursor: pointer;
  transition: all 0.2s;
  position: relative;
}
.tab-btn:hover {
  color: #4f6ef7;
  background: rgba(255, 255 , 255, 0.05);
}
.tab-btn.active {
  color: #fff;
  background: rgba(79, 110, 247, 0.15);
}

.tab-content {
  flex: 1;
  overflow: hidden;
}

.catalog-grid {
  display: grid;
  margin-top: 10px;
  grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
  gap: 16px;
}

.catalog-card {
  background: rgba(26, 33, 43, 0.25);
  backdrop-filter: blur(10px) saturate(180%);
  -webkit-backdrop-filter: blur(10px) saturate(180%);
  border-radius: 16px;
  box-shadow: 0 15px 30px rgba(0, 0, 0, 0.15);
  padding: 20px 16px;
  border: 1px solid #2a3340;
  transition: border-color 0.2s, transform 0.2s;
  cursor: pointer;
  padding-bottom: 30px;
  text-align: center;
}
.catalog-card:hover {
  border-color: #4f6ef7;
  transform: translateY(-5px);
}

.catalog-icon {
  font-size: 2.4rem;
  margin-bottom: 8px;
}

.catalog-title {
  font-weight: bold;
  font-size: 1.4rem;
}

.catalog-count {
  margin-top: 4px;
  color: #8a99b4;
  font-size: 1rem;
}
</style>