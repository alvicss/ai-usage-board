<!--
  檔案：components/QuotaRow.vue
  用途：一個額度一行（玻璃、簡約、霓虹主題）：標籤｜漸變進度條｜已用 %（查詢完成時從舊值滾動到新值），下一行是重置時間。
        進度條長度與顏色都照畫面顯示的整數百分比（displayPercent），和數字一致。
        超過警示門檻時這一行以 3 秒週期呼吸泛紅（FR-7）；精簡模式隱藏重置時間，改用滑鼠停留提示。
        工程師主題用 TerminalRow、儀表主題用 GaugeRing。
  誰會用到：ProviderCard。
-->
<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useRollingPercent } from "../composables/useRollingPercent";
import { quotaLabel, resetText } from "../i18n/format";
import type { ColorScheme, ColorSettings, ResetTimeFormat } from "../types/settings";
import type { Quota } from "../types/usage";
import { numberColor, usageColor } from "../utils/color";
import { displayPercent, isQuotaAlert } from "../utils/quota";
import UsageBar from "./UsageBar.vue";

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

const shownPercent = useRollingPercent(() => props.quota.used_percent);
/** 畫面顯示的整數百分比：條長與漸變顏色都跟著這個數字走（0.06% 顯示 0%，就完全不畫填色） */
const percent = computed(() => displayPercent(props.quota.used_percent));
const label = computed(() => quotaLabel(t, props.quota));
const barColor = computed(() => usageColor(percent.value, props.colors));
const percentColor = computed(() => numberColor(percent.value, props.colors, props.scheme));
const reset = computed(() => resetText(t, props.quota.resets_at, props.resetFormat, props.now));
const alert = computed(() => !props.failed && isQuotaAlert(props.quota, props.threshold));

/** 精簡模式下，把重置時間放進滑鼠停留提示 */
const tooltip = computed(() => {
  if (!props.compact || reset.value === null) {
    return undefined;
  }
  return `${label.value} · ${reset.value.text}`;
});
</script>

<template>
  <div class="quota-row" :class="{ alert }" :title="tooltip">
    <span class="quota-label">{{ label }}</span>
    <UsageBar :percent="percent" :color="barColor" />
    <span class="quota-percent" :style="{ color: percentColor }">{{ shownPercent }}<small>%</small></span>
    <span v-if="reset !== null && !compact" class="reset-time" :class="{ expired: reset.expired }">
      {{ reset.text }}
    </span>
  </div>
</template>
