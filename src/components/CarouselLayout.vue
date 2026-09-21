<!--
  檔案：components/CarouselLayout.vue
  用途：輪播排版（FR-4）：一次顯示一家，每家停留設定的秒數；切換動畫為左右滑動或 3D 翻牌
        （Windows 關閉動畫效果時改成 200ms 淡入淡出）；滑鼠停在小工具上時暫停；
        下方小圓點代表各家，點一下直接跳過去，有警示的那一家圓點顯示紅色；只有一家時不輪播也不顯示圓點。
  誰會用到：WidgetView。
-->
<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { ColorScheme, Settings } from "../types/settings";
import type { ProviderUsage } from "../types/usage";
import type { WidgetLayout } from "../utils/layout";
import { providerHasAlert } from "../utils/quota";
import ProviderCard from "./ProviderCard.vue";

const props = defineProps<{
  providers: ProviderUsage[];
  settings: Settings;
  scheme: ColorScheme;
  now: number;
  layout: WidgetLayout;
  /** 滑鼠停在小工具上（整個小工具，含右上角按鈕）時暫停輪播 */
  paused: boolean;
}>();
const { t } = useI18n();

const currentIndex = ref(0);
const animating = ref(false);
let timer = 0;

/** Windows「動畫效果」是否關閉（關閉時改用淡入淡出） */
const reducedMotionQuery = window.matchMedia("(prefers-reduced-motion: reduce)");
const reducedMotion = ref(reducedMotionQuery.matches);
function onReducedMotionChange(event: MediaQueryListEvent) {
  reducedMotion.value = event.matches;
}

const current = computed(() => {
  if (props.providers.length === 0) {
    return null;
  }
  const safeIndex = currentIndex.value % props.providers.length;
  return props.providers[safeIndex];
});

const transitionName = computed(() => {
  if (reducedMotion.value) {
    return "fade";
  }
  return props.settings.display.carousel_animation;
});

/** 換到下一家 */
function showNext() {
  if (props.providers.length < 2) {
    return;
  }
  currentIndex.value = (currentIndex.value + 1) % props.providers.length;
}

/** 點圓點直接跳到某一家，並重新計時 */
function jumpTo(index: number) {
  currentIndex.value = index;
  restartTimer();
}

/** 依設定秒數重新開始輪播計時；只有一家時不輪播 */
function restartTimer() {
  window.clearInterval(timer);
  if (props.providers.length < 2) {
    return;
  }
  const intervalMs = props.settings.display.carousel_seconds * 1000;
  timer = window.setInterval(() => {
    if (!props.paused) {
      showNext();
    }
  }, intervalMs);
}

watch(() => props.settings.display.carousel_seconds, restartTimer);
watch(
  () => props.providers.length,
  (length) => {
    if (currentIndex.value >= length) {
      currentIndex.value = 0;
    }
    restartTimer();
  },
);

onMounted(() => {
  reducedMotionQuery.addEventListener("change", onReducedMotionChange);
  restartTimer();
});
onBeforeUnmount(() => {
  reducedMotionQuery.removeEventListener("change", onReducedMotionChange);
  window.clearInterval(timer);
});
</script>

<template>
  <section class="carousel">
    <div class="carousel-viewport" :class="{ animating }">
      <div class="carousel-track">
        <Transition
          :name="transitionName"
          @before-leave="animating = true"
          @after-leave="animating = false"
          @leave-cancelled="animating = false"
        >
          <ProviderCard
            v-if="current"
            :key="current.id"
            :provider="current"
            :settings="settings"
            :scheme="scheme"
            :now="now"
            :scale="layout.scale"
            :card-width="layout.cardWidth"
            :card-height="layout.cardHeight"
            :force-compact="layout.forceCompact"
            :reserve-toolbar="true"
          />
        </Transition>
      </div>
    </div>
    <nav v-if="providers.length > 1" class="carousel-dots">
      <button
        v-for="(provider, index) in providers"
        :key="provider.id"
        type="button"
        :class="{ active: current !== null && provider.id === current.id, alert: providerHasAlert(provider, settings) }"
        :aria-label="t('widget.go_to', { name: provider.name })"
        :title="provider.name"
        @click="jumpTo(index)"
      ></button>
    </nav>
  </section>
</template>
