<script setup lang="ts">
import { useI18n } from 'vue-i18n';

import logoUrl from '../assets/logos/wishes.svg';

defineEmits<{
  (e: 'back'): void;
}>();

const { t } = useI18n();

const version = __APP_VERSION__;
const github_url = 'https://github.com/wishes-gacha/wishes';
</script>

<template>
  <div class="about-page">
    <div class="about-header">
      <button class="btn-back" @click="$emit('back')">
        <span class="arrow">←</span> {{ t('common.back') }}
      </button>
    </div>

    <div class="about-scroll">
      <!-- 图标与名称 -->
      <div class="hero-section">
        <img :src="logoUrl" alt="Wishes Logo" class="logo"/>
        <div class="app-name">Wishes 众愿</div>
        <div class="app-version">v{{ version }}</div>
      </div>

      <!-- 项目简介 -->
      <div class="section">
        <div class="section-title">{{ t('about.introTitle') }}</div>
        <div class="section-body">
          <i18n-t keypath="about.introP1" tag="p">
            <template #brand><strong>Wishes (众愿)</strong></template>
          </i18n-t>
          <p>{{ t('about.introP2') }}</p>
        </div>
      </div>

      <!-- 开源协议 -->
      <div class="section">
        <div class="section-title">{{ t('about.licenseTitle') }}</div>
        <div class="section-body">
          <i18n-t keypath="about.licenseP1" tag="p">
            <template #license><strong>GNU General Public License v3.0 or later</strong></template>
          </i18n-t>
          <p>{{ t('about.licenseP2') }}</p>
        </div>
      </div>

      <!-- 项目地址 -->
      <div class="section">
        <div class="section-title">{{ t('about.repoTitle') }}</div>
        <div class="section-body">
          <p>
            {{ t('about.repoLabel') }}
            <a :href="github_url" target="_blank" rel="noopener noreferrer">
              {{ github_url }}
            </a>
          </p>
        </div>
      </div>

      <div class="footer-spacer">
        <sub class="copyright-sub">Copyright © 2026 G-GAZE and contributors · GPL-3.0-or-later</sub>
      </div>
    </div>
  </div>
</template>

<style scoped>
.about-page {
  padding-top: 40px;
  padding-bottom: 16px;
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.about-header {
  flex-shrink: 0;
  padding: 12px 16px 0;
}

.btn-back {
  background: transparent;
  border: none;
  color: #b0c0d0;
  font-size: 1.4rem;
  cursor: pointer;
  padding: 6px 10px;
  border-radius: 6px;
  transition: background 0.2s, color 0.2s;
  user-select: none;
  font-family: inherit;
}
.btn-back:hover {
  background: rgba(255, 255, 255, 0.06);
  color: #fff;
}
.btn-back .arrow {
  margin-right: 4px;
}

.about-scroll {
  flex: 1;
  overflow-y: auto;
  padding: 16px 24px 40px;
  scrollbar-width: thin;
  scrollbar-color: #2a3340 transparent;
}
.about-scroll::-webkit-scrollbar {
  width: 4px;
}
.about-scroll::-webkit-scrollbar-thumb {
  background: #2a3340;
  border-radius: 4px;
}

/* 图标与名称区域 */
.hero-section {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 24px 0 32px;
}

.logo {
  width: 128px;
  height: 128px;
  margin-bottom: 16px;
  filter: drop-shadow(0 4px 16px rgba(79, 110, 247, 0.3));
  user-select: none;
  -webkit-user-drag: none;
  transition: all 0.2s;
}
.logo:hover {
  filter: drop-shadow(0 4px 16px rgba(79, 110, 247, 0.5));
}

.app-name {
  font-size: 2.5rem;
  font-weight: bold;
  color: #e4e8ef;
  letter-spacing: 0.5px;
  user-select: none;
}

.app-version {
  font-size: 1.4rem;
  color: #6a7a90;
  margin-top: 4px;
  user-select: none;
}

/* 通用小节 */
.section {
  background: rgba(26, 33, 43, 0.15);
  backdrop-filter: blur(10px) saturate(180%);
  -webkit-backdrop-filter: blur(10px) saturate(180%);
  border: 1px solid #2a3340;
  border-radius: 12px;
  padding: 16px 20px;
  margin-bottom: 16px;
}

.section-title {
  font-size: 1.4rem;
  font-weight: 600;
  color: #b0c0d0;
  margin-bottom: 12px;
  user-select: none;
}

.section-body {
  font-size: 1.3rem;
  color: #e4e8ef;
  line-height: 1.6;
}

.section-body p {
  margin: 6px 0;
}
.section-body p:first-child {
  margin-top: 0;
}
.section-body p:last-child {
  margin-bottom: 0;
}

.section-body a {
  color: #007bff;
  text-decoration: none;
  word-break: break-all;
}
.section-body a:hover {
  text-decoration: underline;
}

.section-body code {
  background: rgba(79, 110, 247, 0.15);
  color: #b0c0d0;
  padding: 1px 6px;
  border-radius: 4px;
  font-family: 'JetBrains Mono', ui-monospace, monospace;
  font-size: 1.2rem;
}

.footer-spacer {
  display: flex;
  height: 20px;
  justify-content: center;
}

.copyright-sub {
  font-size: 1.2rem;
  color: #6a7a90;
}
</style>