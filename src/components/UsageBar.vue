<!--
  檔案：components/UsageBar.vue
  用途：額度進度條（玻璃、簡約、霓虹主題的橫條）。整條單色，顏色由已用 % 漸變而來；寬度變化有 600ms 過渡。
        percent 是畫面顯示的整數百分比（utils/quota.ts 的 displayPercent），條長跟著數字走；
        0% 時完全不畫填色（圓頭與柔光也不畫），才不會「數字 0% 卻有一小點顏色」。
        工程師主題的字元條在 CharBar.vue，儀表主題的環狀表在 GaugeRing.vue。
  誰會用到：QuotaRow。
-->
<script setup lang="ts">
import { computed } from "vue";

const props = defineProps<{
  /** 畫面顯示的整數百分比（0～100） */
  percent: number;
  /** 填色（漸變色） */
  color: string;
}>();

/** 寬度限制在 0～100% */
const width = computed(() => {
  let value = props.percent;
  if (value < 0) {
    value = 0;
  }
  if (value > 100) {
    value = 100;
  }
  return `${value}%`;
});

/** 0% 時不畫填色 */
const empty = computed(() => props.percent <= 0);
</script>

<template>
  <div class="usage-track" aria-hidden="true">
    <div
      class="usage-fill"
      :class="{ empty }"
      :style="{ width: width, backgroundColor: color, '--fill-color': color }"
    ></div>
  </div>
</template>
