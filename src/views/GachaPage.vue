<script setup lang="ts">
/**
 * 抽卡主页面, 显示当前卡池的详细信息并提供抽卡按钮
 * 通过 `props` 接收 `bannerInfo`, 通过 `emit` 向父组件 (`App.vue`) 传递抽卡事件
 */

import type { BannerInfo } from '../types';
import { useI18n } from 'vue-i18n';
import { pickLocalized, useLocaleText } from '../composables/useLocaleText';

defineProps<{
  bannerInfo: BannerInfo | null
}>()

defineEmits<{
  (e: 'wishSingle'): void,
  (e: 'wishTen'): void,
}>();

const { t } = useI18n();
const { currentLocale } = useLocaleText();
</script>


<template>
  <div class="page gacha-page">
      <div class="gacha-card">
        <div class="info-section">
          <div class="banner-name">{{ bannerInfo ? pickLocalized(bannerInfo.name, currentLocale) : t('gacha.noBannerSelected') }}</div>
          <div class="banner-tags" v-if="bannerInfo">
            <span
              v-for="tag in bannerInfo.tags"
              :key="`${tag.namespace}:${tag.value}`"
              class="tag"
            >
              {{ tag.value }}
            </span>
          </div>
          <div class="detail-list" v-if="bannerInfo">
            <div class="detail-item">
              <span class="label">{{ t('gacha.deckLabel') }}</span>
              <span class="value">{{ pickLocalized(bannerInfo.deck_name, currentLocale) }}</span>
            </div>
            <div class="detail-item">
              <span class="label">{{ t('gacha.logicLabel') }}</span>
              <span class="value">{{ pickLocalized(bannerInfo.logic_name, currentLocale) }}</span>
            </div>
            <div class="detail-item">
              <span class="label">{{ t('gacha.totalCounterLabel') }}</span>
              <span class="value">{{ bannerInfo.total_counter }}</span>
            </div>
          </div>
          <div v-else class="loading_placeholder">{{ t('gacha.loadingInfo') }}</div>
        </div>

        <div class="portrait-section">
          <div class="portrait-placeholder"> Wishes Banner </div>
        </div>
      </div>

    <div class="action-bar">
      <button class="btn-wish btn-single" @click="$emit('wishSingle')" :disabled="!bannerInfo">
        {{ t('gacha.wishSingle') }}
      </button>
      <button class="btn-wish btn-ten" @click="$emit('wishTen')" :disabled="!bannerInfo">
        {{ t('gacha.wishTen') }}
      </button>
    </div>
  </div>
</template>


<style scoped>
.gacha-page {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: 20px 16px 16px;
  gap: 20px;
  background-image:
  radial-gradient(
    ellipse at 10% 90%,
    rgba(77, 109, 255, 0.1) 0%,
    rgba(120, 80, 240, 0.06) 40%,
    transparent 70%
  );
  /* 互补光晕 (舍弃) */
  /* radial-gradient(
    ellipse at 90% 10%,
    rgba(0, 255, 255, 0.03) 0%,
    rgba(0, 200, 255, 0.01) 30%,
    transparent 70%
  ); */
  /* 红配色参考 */
  /* background-image: radial-gradient(
    ellipse at 10% 90%,
    rgba(243, 28, 28, 0.06),
    transparent 70%
  ); */
}

.gacha-card {
  flex: 1;
  display: flex;
  border-radius: 20px;
  box-shadow: 0 15px 30px rgba(0, 0, 0, 0.15);
  position: relative;
  min-height: 250px;
  min-width: 450px;
}

.info-section {
  flex: 0 0 40%;
  padding: 32px 28px 28px 28px;
  display: flex;
  flex-direction: column;
  justify-content: center;
  z-index: 2;
  position: relative;
  border-radius: 0 40px 0 20px;
}

.banner-name {
  font-size: 2.4rem;
  font-weight: bold;
  color: #eef2f8;
  margin-bottom: 6px;
  line-height: 1.2;
  text-shadow: 0 2px 8px rgba(255, 255, 255, 0.35);
}

.banner-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 18px;
}

.tag {
  background: rgba(255, 255, 255, 0.06);
  padding: 2px 14px;
  border-radius: 100px;
  font-size: 1.2rem;
  color: #a8b8d0;
  letter-spacing: 0.3px;
}

.detail-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.detail-item {
  display: flex;
  justify-content: space-between;
  padding: 6px 0;
  border-bottom: 1px solid #2a3340;
}

.detail-item:last-child {
  border-bottom: none;
}

.detail-item .label {
  /* color: #8a99b4; */
  color: #7a8aa0;
  font-size: 1.2rem;
}

.detail-item .value {
  color: #e4e8ef;
  font-size: 1.2rem;
}

/* 阴影分隔 */
.info-section {
  box-shadow: 4px 0 30px rgba(0, 0, 0, 0.3);
}

.portrait-section {
  flex: 0 0 60%;
}

.portrait-placeholder {
  position: absolute;
  bottom: 20px;
  right: 20px;
  font-size: 3rem;
  font-family: system-ui;
  font-weight: bold;
  background: transparent;
  transform: skewX(-15deg);
  color: #fff;
  text-shadow: 0 2px 8px rgba(255, 255, 255, 0.35);
  user-select: none;
}

.action-bar {
  display: flex;
  gap: 16px;
  align-items: flex-end;
  justify-content: flex-end;
  flex-shrink: 0;
  padding: 0 4px;
}

.btn-wish {
  flex: 1;
  max-width: 200px;
  padding: 16px 24px;
  border: none;
  border-radius: 14px;
  font-size: 1.5rem;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  position: relative;
  overflow: hidden;
  user-select: none;
}
.btn-wish:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.btn-single {
  background: #2a3340;
  color: #e4e8ef;
}
.btn-single:hover:not(:disabled) {
  background: #3a4555;
}

.btn-ten {
  background: #007bff;
  color: #fff;
}
.btn-ten:hover:not(:disabled) {
  background: #3593ff;
  transform: scale(1.02);
}
</style>