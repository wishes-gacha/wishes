<script setup lang="ts">
/**
 * 抽卡结果浮层, 支持单抽/十连两种模式
 * - 单抽: 直接展示卡片内容和标签, 点击关闭
 * - 十连: 逐张展示, 点击继续下一张, 全部展示后进入汇总统计
 * 包含阶段切换、点击提示和跳过
 */

import { computed, onUnmounted, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { WishResponse } from '../types';
import { useTagStyles } from '../composables/useTagStyles';
import { pickLocalized, useLocaleText } from '../composables/useLocaleText';

const props = defineProps<{
  result: WishResponse | null,  // 单抽结果 (仅单抽时使用)
  isTenWish: boolean,
  tenResults?: WishResponse[],   // 十连结果数组
}>();

const emit = defineEmits<{
  (e: 'close'): void
}>();

const { t } = useI18n();
const { currentLocale } = useLocaleText();

const timerInterval = 2000;   // 点击提示出现的延迟时间

// 阶段控制
const phase = ref<'playing' | 'summary' | 'closing'>('playing');
const isClosing = ref(false);


// 十连专用状态
const currentIndex = ref(0);
let continueHintTimer: ReturnType<typeof setTimeout> | null = null;
let closeHintTimer: ReturnType<typeof setTimeout> | null = null;

// 当前展示卡片
const currentCard = computed(() => {
  if (props.isTenWish && props.tenResults) {
    return props.tenResults[currentIndex.value] || null;
  }
  return null;
})

const isTenCompleted = computed(() => {
  if (!props.isTenWish || !props.tenResults) return false;
  const res = currentIndex.value >= props.tenResults.length - 1;
  return res;
})

function goToSummary() {
  if (phase.value === 'playing') {
    phase.value = 'summary';
    showContinueHint.value = false;
    if (continueHintTimer) clearTimeout(continueHintTimer);
    restartCloseHintTimer();
  }
}

function closeOverlay() {
  if (isClosing.value) return;
  isClosing.value = true;
  setTimeout(() => {
    emit('close');
  }, 300);
}

const showContinueHint = ref(false);
const showCloseHint = ref(false);

/** 汇总统计: 按稀有度计数 */
const summaryStats = computed(() => {
  if (!props.tenResults) return null;
  const stats: Record<string, number> = {};
  props.tenResults.forEach(r => {
    const rarityTag = r.tags.find(t => t.namespace === "rarity");
    const rarity = rarityTag ? rarityTag.value : "unknown";
    stats[rarity] = (stats[rarity] || 0) + 1;
  })
  return stats
});

function restartCloseHintTimer() {
  if (closeHintTimer) clearTimeout(closeHintTimer);
  closeHintTimer = setTimeout(() => {
    showCloseHint.value = true;
  }, timerInterval)
}

function restartContinueHintTimer() {
  if (continueHintTimer) clearTimeout(continueHintTimer);
  if (!props.isTenWish) return;
  continueHintTimer = setTimeout(() => {
    showContinueHint.value = true;
  }, timerInterval)
}

// 根据是否十连决定显示哪种提示
watch(() => props.isTenWish, (newVal) => {
  if (newVal) {
    restartContinueHintTimer();
  } else {
    restartCloseHintTimer();
  }
}, { immediate: true });

onUnmounted(() => {
  if (continueHintTimer) clearTimeout(continueHintTimer);
  if (closeHintTimer) clearTimeout(closeHintTimer);
})

/** 处理点击, 单抽关闭, 十连继续/展示汇总结果/关闭 */
function handleOverlayClick() {
  if (isClosing.value) return;
  if (phase.value === 'summary') {
    closeOverlay();
    return;
  }
  if (props.isTenWish) {
    if (isTenCompleted.value) {
      phase.value = 'summary';
      showContinueHint.value = false;
      if (continueHintTimer) clearTimeout(continueHintTimer);
      restartCloseHintTimer();
    } else {
      if (currentIndex.value < (props.tenResults?.length || 0) - 1) {
        currentIndex.value++;
        showContinueHint.value = false;
        restartContinueHintTimer();
      }
    }
  } else {
    closeOverlay();
  }
}

/** 十连模式下跳过, 直接展示汇总结果 */
function handleSkip() {
  if (isClosing.value) return;
  if (phase.value == 'playing') {
    goToSummary();
  }
}

/** 获取稀有度标签值 */
function getRarity(item: WishResponse): string {
  const tag = item.tags.find(t => t.namespace === "rarity");
  return tag ? tag.value : "unknown";
}

/** 卡片在当前界面语言下的显示内容. */
function displayContent(item: WishResponse | null | undefined): string {
  return item ? pickLocalized(item.content, currentLocale.value) : '';
}

// 单抽模式下的标签类名
const singleTagClasses = computed(() => {
  if (!props.result) return [];
  return useTagStyles(props.result.tags);
});

// 十连模式下当前卡片的标签类名
const currentTagClasses = computed(() => {
  if (!props.isTenWish || !props.tenResults) return [];
  return useTagStyles(currentCard.value?.tags || []);
});
</script>


<template>
  <div class="overlay" :class="{ closing: isClosing }" @click="handleOverlayClick">
    <div class="overlay-content">

      <!-- 单抽 -->
      <template v-if="!isTenWish && result">
        <div class="card-content animate-in" @click.stop>
          {{ displayContent(result) }}
        </div>
        <div v-if="result.tags.length" class="tags" @click.stop>
          <span
            v-for="(tag, index) in result.tags"
            :key="`${tag.namespace}:${tag.value}`" class="tag"
            :class="singleTagClasses[index]"
          >
            {{ tag.value }}
          </span>
        </div>
        <div v-if="result.event_tags.length" class="event-tags" @click.stop>
          <span v-for="et in result.event_tags" :key="et" class="event-tag" :class="`tag-event-${et}`">
            {{ et }}
          </span>
        </div>
      </template>

      <template v-if="isTenWish && tenResults?.length">
        <Transition name="ten-player-summary-switch" mode="out-in">
          <!-- 播放 -->
          <div v-if="phase === 'playing'" key="playing" class="ten-player">
            <div class="ten-counter">
              {{ currentIndex + 1 }} / {{ tenResults.length }}
            </div>
            <Transition name="card-switch" mode="out-in">
              <div :key="currentIndex" class="card-wrapper">
                <div class="card-content animate-in" @click.stop>
                  {{ displayContent(currentCard) }}
                </div>
                <div v-if="currentCard?.tags.length" class="tags" @click.stop>
                  <span
                    v-for="(tag, index) in currentCard.tags"
                    :key="`${tag.namespace}:${tag.value}`"
                    class="tag"
                    :class="currentTagClasses[index]"
                  >
                    {{ tag.value }}
                  </span>
                </div>
                <div v-if="currentCard?.event_tags.length" class="event-tags" @click.stop>
                  <span v-for="et in currentCard.event_tags" :key="et" class="event-tag" :class="`tag-event-${et}`">
                    {{ et }}
                  </span>
                </div>
              </div>
            </Transition>
          </div>

          <!-- 汇总 -->
          <div v-else-if="phase === 'summary'" key="summary">
            <div class="summary-stats" @click.stop>
              <div v-for="(count, rarity) in summaryStats" :key="rarity" class="stat-item" :class="`tag-rarity-${rarity}`">
                {{ t('wish.rarity', { rarity, count }) }}
              </div>
            </div>
            <div class="summary-list" @click.stop>
              <div v-for="(item, idx) in tenResults" :key="idx" class="summary-item" :class="`tag-rarity-${getRarity(item)}`">
                {{ displayContent(item) }}
              </div>
            </div>
          </div>
        </Transition>
      </template>
      <div v-if="isTenWish && tenResults?.length && phase == 'playing'" class="skip-btn" @click.stop="handleSkip">
        {{ t('wish.skip') }}
      </div>
      <div v-if="showContinueHint" class="click-hint">{{ t('wish.clickContinue') }}</div>
      <div v-if="showCloseHint" class="click-hint">{{ t('wish.clickClose') }}</div>
    </div>
  </div>
</template>


<style scoped>
@keyframes popIn {
  0% { transform: scale(0.5); opacity: 0; }
  100% { transform: scale(1); opacity: 1; }
}

@keyframes fadeIn {
  0% { opacity: 0; }
  100% { opacity: 1; }
}

@keyframes fadeOut {
  0% { opacity: 1; }
  100% { opacity: 0; }
}

.overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.9);
  backdrop-filter: blur(12px);
  z-index: 999;                         /* 覆盖整个界面 */
  display: flex;
  align-items: center;
  justify-content: center;
  animation: fadeIn 0.4s ease;
}
.overlay.closing {
  animation: fadeOut 0.3s ease forwards;
}

.overlay-content {
  text-align: center;
  padding: 40px;
  max-width: 600px;
  width: 90%;
}

.skip-btn {
  position: fixed;
  top: 6px;
  right: 0;
  color: #b0c0d0;
  padding: 4px 16px;
  cursor: pointer;
  font-size: 1.5rem;
}

.ten-counter {
  font-size: 1.5rem;
  color: #8a99b4;
  margin-bottom: 16px;
  letter-spacing: 2px;
  user-select: none;
}

.card-wrapper {
  display: flex;
  flex-direction: column;
  align-items: center;
}

.card-content {
  font-size: 4rem;
  font-weight: bold;
  color: #fff;
  margin-bottom: 12px;
  text-shadow: 0 2px 15px rgba(255, 255, 255, 0.5);
}

.animate-in {
  animation: popIn 0.5s cubic-bezier(0.34, 1.56, 0.64, 1);
}

.card-switch-enter-active,
.card-switch-leave-active {
  transition: opacity 0.25s ease, transform 0.25s ease;
}
.card-switch-enter-from {
  opacity: 0;
  transform: scale(0.8);
}
.card-switch-leave-to {
  opacity: 0;
  transform: scale(1.2);
}

.ten-player-summary-switch-enter-active,
.ten-player-summary-switch-leave-active {
  transition: opacity 0.3s ease, transform 0.3s ease;
}
.ten-player-summary-switch-enter-from {
  opacity: 0;
  transform: scale(0.8);
}
.ten-player-summary-switch-leave-to {
  opacity: 0;
  transform: scale(1.2);
}

.tags {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: 8px;
  margin-top: 8px;
}

.tag {
  background: var(--tag-bg, var(--tag-default-bg));
  color: var(--tag-color, var(--tag-default-color));

  padding: 3px 16px;
  border-radius: 100px;
  font-size: 1.4rem;
  transition: all 0.2s ease;
}
.tag:hover {
  transform: translateY(-3px);
}

.event-tags {
  margin-top: 12px;
}

.event-tag {
  background: var(--tag-bg, var(--tag-default-bg));
  color: var(--tag-color, var(--tag-default-color));
  display: inline-block;
  padding: 3px 20px;
  border-radius: 100px;
  font-weight: bold;
  font-size: 1.4rem;
  transition: all 0.2s ease;
}
.event-tag:hover {
  transform: translateY(-3px);
}

.click-hint {
  position: absolute;
  left: calc((50% - 100px));
  bottom: 40px;
  width: 200px;
  height: 20px;
  text-align: center;
  margin-top: 40px;
  color: #6a7a90;
  font-size: 1.3rem;
  letter-spacing: 1px;
  user-select: none;
  pointer-events: none;
  animation: fadeIn 0.6s ease;
  text-shadow: 0 2px 25px rgba(255, 255, 255, 0.9);
}

.summary-stats {
  display: flex;
  justify-content: center;
  gap: 20px;
  margin-bottom: 20px;
}

.stat-item {
  background: var(--tag-bg, var(--tag-default-bg));
  color: var(--tag-color, var(--tag-default-color));

  backdrop-filter: blur(4px);
  padding: 6px 20px;
  border-radius: 100px;
  font-size: 1.5rem;
  font-weight: 600;
  box-shadow: 0 0 20px rgba(255, 215, 0, 0.1);
  transition: all 0.2s ease;
}
.stat-item:hover {
  transform: translateY(-3px);
}

.summary-list {
  max-height: 300px;
  overflow-y: auto;
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  justify-content: center;
  padding-top: 4px;
}

.summary-item {
  background: var(--tag-bg, var(--tag-default-bg));
  color: var(--tag-color, var(--tag-default-color));

  padding: 4px 16px;
  border-radius: 100px;
  font-size: 1.3rem;
  transition: all 0.2s ease;
}
.summary-item:hover {
  transform: translateY(-3px);
}
</style>