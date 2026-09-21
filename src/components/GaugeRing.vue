<!--
  檔案：components/GaugeRing.vue
  用途：儀表主題的一個額度一個環狀表（43「gauge 儀表」，圖形與畫面示意相同）：
        270° 的弧、缺口朝下，填色用漸變色；0／60／80／100% 四個小刻度；環中央是大號百分比（查詢完成時滾動到新值），
        環下方是標籤與重置時間（短格式）。超過警示門檻時環呼吸閃爍、底色泛警示色（FR-7、FR-18：顏色取 --w-alert）。
        環的大小跟著卡片縮放；精簡模式不顯示重置時間，改放進滑鼠停留提示。
        弧長與顏色照畫面顯示的整數百分比（displayPercent）；0% 時不畫填色弧（圓頭會變成一個點）。
  誰會用到：ProviderCard（主題是 gauge 時）。
-->
<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRollingPercent } from "../composables/useRollingPercent";
import { compactResetText, quotaLabel } from "../i18n/format";
import type { ColorScheme, ColorSettings, ResetTimeFormat } from "../types/settings";
import type { Quota } from "../types/usage";
import { numberColor, usageColor } from "../utils/color";
import { displayPercent, isQuotaAlert } from "../utils/quota";

const props = defineProps<{
  quota: Quota;
  colors: ColorSettings;
  scheme: ColorScheme;
  threshold: number;
  resetFormat: ResetTimeFormat;
  now: number;
  compact: boolean;
  failed: boolean;
}>();
const { t } = useI18n();

// ---------- 環的幾何（SVG 座標，viewBox 與畫面示意相同） ----------
/** 環的圓心與半徑 */
const CENTER = 50;
const RADIUS = 40;
/** 整圈長度、270° 弧的長度（缺口 90° 朝下） */
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;
const ARC_LENGTH = CIRCUMFERENCE * 0.75;
/** 刻度的位置（佔整條弧的比例）：0%、60%、80%、100% */
const TICK_POSITIONS = [0, 0.6, 0.8, 1];
/** 刻度線從半徑 46 畫到 51（在環的外側） */
const TICK_INNER_RADIUS = 46;
const TICK_OUTER_RADIUS = 51;
/** 弧從左下 135° 開始，順時針畫 270° */
const START_ANGLE_DEGREES = 135;

const shownPercent = useRollingPercent(() => props.quota.used_percent);
/** 畫面顯示的整數百分比：弧長與漸變顏色都跟著這個數字走 */
const percent = computed(() => displayPercent(props.quota.used_percent));
const label = computed(() => quotaLabel(t, props.quota));
const ringColor = computed(() => usageColor(percent.value, props.colors));
const percentColor = computed(() => numberColor(percent.value, props.colors, props.scheme));
const reset = computed(() => compactResetText(t, props.quota.resets_at, props.resetFormat, props.now));
const alert = computed(() => !props.failed && isQuotaAlert(props.quota, props.threshold));

/** 填色弧的長度：顯示的整數百分比乘上 270° 弧長（displayPercent 已限制在 0～100） */
const filledLength = computed(() => (ARC_LENGTH * percent.value) / 100);

/** 軌道與填色都用 stroke-dasharray 畫出「一段弧＋其餘空白」 */
const trackDash = `${ARC_LENGTH.toFixed(1)} ${CIRCUMFERENCE.toFixed(1)}`;
const fillDash = computed(() => `${filledLength.value.toFixed(1)} ${CIRCUMFERENCE.toFixed(1)}`);

/** 四個刻度線的兩端座標 */
const ticks = computed(() => {
  const lines = [];
  for (const position of TICK_POSITIONS) {
    const angle = ((START_ANGLE_DEGREES + 270 * position) * Math.PI) / 180;
    lines.push({
      x1: (CENTER + TICK_INNER_RADIUS * Math.cos(angle)).toFixed(1),
      y1: (CENTER + TICK_INNER_RADIUS * Math.sin(angle)).toFixed(1),
      x2: (CENTER + TICK_OUTER_RADIUS * Math.cos(angle)).toFixed(1),
      y2: (CENTER + TICK_OUTER_RADIUS * Math.sin(angle)).toFixed(1),
    });
  }
  return lines;
});

/** 精簡模式下，把重置時間放進滑鼠停留提示 */
const tooltip = computed(() => {
  if (!props.compact || reset.value === null) {
    return undefined;
  }
  return `${label.value} · ${reset.value.text}`;
});
</script>

<template>
  <div class="gauge-ring" :class="{ alert }" :title="tooltip">
    <svg viewBox="-3 -3 106 94" role="img" :aria-label="`${label} ${t('widget.used', { percent: shownPercent })}`">
      <circle
        class="gauge-track"
        :cx="CENTER"
        :cy="CENTER"
        :r="RADIUS"
        :transform="`rotate(${START_ANGLE_DEGREES} ${CENTER} ${CENTER})`"
        :stroke-dasharray="trackDash"
      />
      <circle
        v-if="percent > 0"
        class="gauge-fill"
        :cx="CENTER"
        :cy="CENTER"
        :r="RADIUS"
        :transform="`rotate(${START_ANGLE_DEGREES} ${CENTER} ${CENTER})`"
        :stroke-dasharray="fillDash"
        :style="{ stroke: ringColor }"
      />
      <line
        v-for="(tick, index) in ticks"
        :key="index"
        class="gauge-tick"
        :x1="tick.x1"
        :y1="tick.y1"
        :x2="tick.x2"
        :y2="tick.y2"
      />
      <text class="gauge-number" x="50" y="57" text-anchor="middle" :style="{ fill: percentColor }">
        {{ shownPercent }}%
      </text>
    </svg>
    <span class="gauge-label">{{ label }}</span>
    <span v-if="reset !== null && !compact" class="gauge-reset" :class="{ expired: reset.expired }">
      {{ reset.text }}
    </span>
  </div>
</template>
