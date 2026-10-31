<script setup lang="ts">
/**
 * 设置页面, 展示应用设置项
 * "关于" 作为列表入口, 通过视图切换展示 AboutPage 子视图
 */

import { invoke } from '@tauri-apps/api/core';
import { onMounted, ref } from 'vue';
import AboutPage from './AboutPage.vue';
import { useMessage, NSelect, type SelectOption } from 'naive-ui';
import { useI18n } from 'vue-i18n';
import { SUPPORTED_LOCALES, setLocale, type Locale } from '../i18n';

const message = useMessage();
const { t, locale } = useI18n();

/** 语言选项 (名称按各自语言显示, 不随界面语言变化). */
const languageOptions: SelectOption[] = [
  { value: 'zh-CN', label: '简体中文' },
  { value: 'en',    label: 'English' },
  { value: 'ja',    label: '日本語' },
];

/** 切换界面语言. */
function onLocaleChange(value: string) {
  if ((SUPPORTED_LOCALES as readonly string[]).includes(value)) {
    setLocale(value as Locale);
  }
}

const loading = ref(false);
const data_dir = ref<string | null>(null);
const log_dir = ref<string | null>(null);

const dataDirOpening = ref(false);
const logDirOpening = ref(false);

/** 视图状态: settings (设置列表) | about (关于页) */
const currentView = ref<'settings' | 'about'>('settings');

onMounted(() => {
  const loadDataDir = async () => {
    loading.value = true;
    try {
      const [d, l] = await Promise.all([
        invoke<string>('get_data_dir_path'),
        invoke<string>('get_log_dir_path'),
      ]);
      data_dir.value = d;
      log_dir.value = l;
    } catch (e) {
      message.error(t('settings.error.loadPaths', { error: e }));
      data_dir.value = null;
      log_dir.value = null;

    } finally {
      loading.value = false;
    }
  };

  loadDataDir();
});

async function openDataDir() {
  if (dataDirOpening.value) return;
  dataDirOpening.value = true;
  try {
    await invoke('open_data_dir');
  } catch (e) {
    message.error(t('settings.error.openDataDir', { error: e }));
  } finally {
    dataDirOpening.value = false;
  }
}

async function openLogDir() {
  if (logDirOpening.value) return;
  logDirOpening.value = true;
  try {
    await invoke('open_log_dir');
  } catch (e) {
    message.error(t('settings.error.openLogDir', { error: e }));
  } finally {
    logDirOpening.value = false;
  }
}
</script>

<template>
  <div class="settings-page-wrapper">
    <!-- 设置视图 -->
    <Transition name="slide-settings">
      <div v-if="currentView === 'settings'" class="settings-page">
        <span class="title">{{ t('settings.title') }}</span>
        <span class="subtitle">{{ t('settings.subtitle') }}</span>

        <div class="settings-list">
          <div class="setting-item">
            <span class="setting-label">{{ t('settings.language') }}</span>
            <span class="setting-value">
              <n-select
                :value="locale"
                :options="languageOptions"
                :consistent-menu-width="false"
                size="small"
                @update:value="onLocaleChange"
              />
            </span>
          </div>
          <div class="setting-item">
            <span class="setting-label">{{ t('settings.dataDir') }}</span>
            <span class="setting-value">
              <span class="path-text" :title="data_dir ?? ''">{{ data_dir ?? t('common.loading') }}</span>
              <button
                v-if="data_dir"
                class="open-btn"
                :disabled="dataDirOpening"
                :title="data_dir ?? ''"
                @click="openDataDir"
              >{{ t('common.open') }}</button>
            </span>
          </div>
          <div class="setting-item">
            <span class="setting-label">{{ t('settings.logDir') }}</span>
            <span class="setting-value">
              <span class="path-text" :title="log_dir ?? ''">{{ log_dir ?? t('common.loading') }}</span>
              <button
                v-if="log_dir"
                class="open-btn"
                :disabled="logDirOpening"
                :title="log_dir ?? ''"
                @click="openLogDir"
              >{{ t('common.open') }}</button>
            </span>
          </div>
          <div class="setting-item">
            <span class="setting-label">{{ t('settings.darkMode') }}</span>
            <span class="setting-value">{{ t('settings.darkModeEnabled') }}</span>
          </div>
          <div class="setting-item clickable" @click="currentView = 'about'">
            <span class="setting-label">{{ t('settings.about') }}</span>
            <span class="setting-value">
              {{ t('settings.aboutSummary') }}
              <span class="arrow">›</span>
            </span>
          </div>
        </div>
      </div>
    </Transition>

    <!-- 关于视图 -->
    <Transition name="slide-about">
      <div v-if="currentView === 'about'" class="about-view-wrapper">
        <AboutPage @back="currentView = 'settings'" />
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.settings-page-wrapper {
  position: relative;
  width: 100%;
  height: 100%;
  overflow: hidden;
  background-image: radial-gradient(
    ellipse at 10% 90%,
    rgba(77, 109, 255, 0.1) 0%,
    rgba(120, 80, 240, 0.06) 40%,
    transparent 70%
  );
}

.settings-page,
.about-view-wrapper {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
}

.settings-page {
  padding: 40px 16px;
  /* background-image: radial-gradient(
    ellipse at 10% 90%,
    rgba(77, 109, 255, 0.1) 0%,
    rgba(120, 80, 240, 0.06) 40%,
    transparent 70%
  ); */
  overflow-y: auto;
}

.title {
  display: block;
  font-size: 2.5rem;
  font-weight: 600;
  user-select: none;
}
.subtitle {
  display: block;
  color: #8a99b4;
  font-size: 1.2rem;
  padding: 10px;
  user-select: none;
}

.settings-list {
  background: rgba(26, 33, 43, 0.1);
  backdrop-filter: blur(10px) saturate(180%);
  -webkit-backdrop-filter: blur(10px) saturate(180%);
  border-radius: 16px;
  border: 1px solid #2a3340;
  overflow: hidden;
  margin-top: 10px;
}

.setting-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 14px 18px;
  border-bottom: 1px solid #2a3340;
  transition: background 0.2s;
}
.setting-item:last-child {
  border-bottom: none;
}

.setting-item.clickable {
  cursor: pointer;
  user-select: none;
}
.setting-item.clickable:hover {
  background: rgba(79, 110, 247, 0.08);
}

.setting-label {
  font-size: 1.3rem;
  color: #e4e8ef;
}

.setting-value {
  color: #8a99b4;
  font-size: 1.1rem;
  display: flex;
  align-items: center;
  gap: 6px;
}
.setting-value .arrow {
  color: #4a5a70;
  font-size: 1.6rem;
  line-height: 1;
}

.path-text {
  max-width: 40ch;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  display: inline-block;
  vertical-align: bottom;
  direction: rtl;
}

.open-btn {
  background: rgba(79, 110, 247, 0.15);
  border: 1px solid #2d3642;
  border-radius: 6px;
  padding: 3px 12px;
  color: #b0c0d0;
  font-size: 1.1rem;
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
  flex-shrink: 0;
}
.open-btn:hover:not(:disabled) {
  background: rgba(79, 110, 247, 0.3);
  color: #fff;
}
.open-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* --- 滑动过渡 --- */
.slide-settings-enter-active,
.slide-settings-leave-active,
.slide-about-enter-active,
.slide-about-leave-active {
  transition: transform 0.4s ease;
}

/* 设置视图: 从左侧滑入 / 向左侧滑出 */
.slide-settings-enter-from,
.slide-settings-leave-to {
  transform: translateX(-100%);
}

/* 关于视图: 从右侧滑入 / 向右侧滑出 */
.slide-about-enter-from,
.slide-about-leave-to {
  transform: translateX(100%);
}
</style>