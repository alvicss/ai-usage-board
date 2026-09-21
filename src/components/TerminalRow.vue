<!--
  檔案：components/TerminalRow.vue
  用途：工程師主題的一個額度一行，像終端機輸出：「! 5 小時  ██████░░░░  62%  ↻ 2h13m」（43「terminal 工程師」）。
        - 超過警示門檻時，行首的「!」用警示色閃爍（FR-7、FR-18：顏色取 --w-alert），不另外泛紅底色；
        - 百分比查詢完成時從舊值滾動到新值；字元條格數與顏色照畫面顯示的整數百分比（displayPercent）；
        - 重置時間用短格式，放在同一行最右邊；精簡模式或卡片太窄時（compact）改放進滑鼠停留提示。
  誰會用到：ProviderCard（主題是 terminal 時）。
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
import CharBar from "./CharBar.vue";

const props = defineProps<{
  quota: Quota;
  colors: ColorSettings;
  scheme: ColorScheme;
  threshold: number;
  resetFormat: ResetTimeFormat;
  now: number;
  /** 不在行尾顯示重置時間（精簡模式，或卡片太窄放不下；改放滑鼠停留提示） */
  compact: boolean;
  failed: boolean;
  /** 字元條總格數（ProviderCard 依卡片寬度算好） */
  cells: number;
}>();
const { t } = useI18n();

const shownPercent = useRollingPercent(() => props.quota.used_percent);
/** 畫面顯示的整數百分比：條長與漸變顏色都跟著這個數字走（0.06% 顯示 0%，就完全不畫填色） */
const percent = computed(() => displayPercent(props.quota.used_percent));
const label = computed(() => quotaLabel(t, props.quota));
const barColor = computed(() => usageColor(percent.value, props.colors));
const percentColor = computed(() => numberColor(percent.value, props.colors, props.scheme));
const reset = computed(() => compactResetText(t, props.quota.resets_at, props.resetFormat, props.now));
const alert = computed(() => !props.failed && isQuotaAlert(props.quota, props.threshold));

/** 行尾的重置時間：「↻ 2h13m」；已重置時只顯示「已重置，等待更新」 */
const resetDisplay = computed(() => {
  if (reset.value === null) {
    return "";
  }
  if (reset.value.expired) {
    return reset.value.text;
  }
  return `↻ ${reset.value.text}`;
});

/** 精簡模式下，把重置時間放進滑鼠停留提示 */
const tooltip = computed(() => {
  if (!props.compact || resetDisplay.value === "") {
    return undefined;
  }
  return `${label.value} · ${resetDisplay.value}`;
});
</script>

<template>
  <div class="quota-row terminal-row" :class="{ alert }" :title="tooltip">
    <span class="quota-label"><span class="terminal-bang">!</span>{{ label }}</span>
    <CharBar :percent="percent" :color="barColor" :cells="cells" />
    <span class="quota-percent" :style="{ color: percentColor }">{{ shownPercent }}<small>%</small></span>
    <span class="reset-time" :class="{ expired: reset !== null && reset.expired }">
      {{ compact ? "" : resetDisplay }}
    </span>
  </div>
</template>
