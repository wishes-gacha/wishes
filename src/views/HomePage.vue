<script setup lang="ts">
/**
 * 主页, 以网格形式展示所有卡池摘要, 点击跳转至抽卡页
 * 包含背景光晕跟随鼠标移动的交互效果
 */

import { ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { BannerSummary } from '../types';
import { pickLocalized, useLocaleText } from '../composables/useLocaleText';

defineEmits<{
  (e: "select-banner", bannerId: string): void
}>();

defineProps<{
  banners: BannerSummary[],
}>();

const { t } = useI18n();
const { currentLocale } = useLocaleText();


// 背景光晕实现
const pageRef = ref<HTMLElement | null>(null);
const glowRef = ref<HTMLElement | null>(null);

/** 鼠标移动时跳转背景光晕位置, 直接视差效果 */
const handleMouseMove = (e: MouseEvent) => {
  if (!pageRef.value || !glowRef.value) return;
  const rect = pageRef.value.getBoundingClientRect();
  const x = (e.clientX - rect.left) / rect.width;
  const y = (e.clientY - rect.top) / rect.height;
  const offsetX = (x - 0.5) * 100;
  const offsetY = (y - 0.5) * 100;
  glowRef.value.style.transform = `translate(${offsetX}%, ${offsetY}%)`;
}

const resetGlow = () => {
  if (glowRef.value) {
    glowRef.value.style.transform = `translate(0%, 0%)`; 
  }
};
</script>


<template>
  <div class="home-page" @mousemove="handleMouseMove" @mouseleave="resetGlow" ref="pageRef">
    <div class="bg-glow" ref="glowRef"></div>

    <div class="header-area">
      <span class="title">{{ t('home.title') }}</span>
    <span class="subtitle">{{ t('home.subtitle') }}</span>
    </div>
    
    <div class="banner-grid" v-if="banners.length">
      <div
        v-for="banner in banners"
        :key="banner.global_id"
        class="banner-card"
        @click="$emit('select-banner', banner.global_id)"
      >
        <div class="banner-thumbnail">
          <div class="thumbnail-placeholder"></div>
        </div>
        <div class="banner-name">{{ pickLocalized(banner.name, currentLocale) }}</div>
        <div class="banner-tags">
          <span
            v-for="tag in banner.tags"
            :key="`${tag.namespace}:${tag.value}`"
            class="tag"
          >
            {{ tag.value }}
          </span>
        </div>
      </div>
    </div>
  </div>
</template>


<style scoped>
.home-page {
  display: flex;
  flex-direction: column;
  position: relative;
  height: 100%;
  padding: 40px 16px 0;
  overflow: hidden;
}

.bg-glow {
  position: fixed;
  top: 0%;
  left: 0%;
  width: 120vmax;
  height: 120vmax;
  background: radial-gradient(
    circle at center,
    rgba(77, 109, 255, 0.35) 0%,
    rgba(120, 80, 240, 0.15) 40%,
    transparent 70%
  );
  filter: blur(90px);
  transform: translate(0%, 0%);
  transition: transform 0.6s ease-out;
  pointer-events: none;
  z-index: 0;
}

.header-area {
  flex-shrink: 0;
  padding-bottom: 12px;
  z-index: 1;
}

.title {
  font-size: 2.5rem;
  font-weight: 600;
  margin-bottom: 4px;
  user-select: none;
}

.subtitle {
  color: #8a99b4;
  font-size: 1.2rem;
  padding: 10px;
  user-select: none;
}

.banner-grid {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: 16px;
  align-content: start;
  overflow-y: auto;
  scrollbar-width: none;
  padding-top: 5px;       /* 为卡片上浮动画预留的空间 */
  padding-bottom: 16px;   /* 底边距 */
}
.banner-grid::-webkit-scrollbar {
  display: none;
}

.banner-card {
  background: rgba(26, 33, 43, 0.25);
  backdrop-filter: blur(10px) saturate(180%);
  -webkit-backdrop-filter: blur(10px) saturate(180%);
  border-radius: 16px;
  box-shadow: 0 15px 30px rgba(0, 0, 0, 0.15);
  padding: 12px;
  border: 1px solid #2a3340;
  border-bottom: 5px solid #2a3340;
  transition: border-color 0.2s, transform 0.2s;
  cursor: pointer;
  padding-bottom: 30px;
  position: relative;
  height: fit-content;
}
.banner-card:hover {
  border-color: #3395ff;
  transform: translateY(-5px);
}
.banner-card:active {
  transform: translateY(5px);
}

.banner-thumbnail {
  aspect-ratio: 16/9;
  border-radius: 8px;
  overflow: hidden;
  margin-bottom: 8px;
}
.thumbnail-placeholder {
  width: 100%;
  height: 100%;
  background: linear-gradient(135deg, #2a3340, #1a212b);
}

.banner-name {
  font-weight: 500;
  font-size: 1.5rem;
}

.banner-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 4px;
}
.tag {
  background: #2a3340;
  padding: 4px 10px;
  border-radius: 100px;
  font-size: 1.2rem;
  color: #b0c0d0;
}

.back-body {
  flex: 1;
  background: radial-gradient(ellipse at 30% 70%, rgba(79,110,247,0.08), transparent 70%);
}

.empty-state {
  color: #6a7a90;
  text-align: center;
  padding: 60px 0;
}
</style>