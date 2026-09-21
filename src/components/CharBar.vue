<!--
  檔案：components/CharBar.vue
  用途：工程師主題的字元進度條（43「terminal 工程師」）：填滿的格子用 █（漸變色），空格用 ░（主題的空格色）。
        percent 是畫面顯示的整數百分比，填滿格數照這個整數算（utils/quota.ts 的 filledCellCount，0% 一定是 0 格）。
        格數由 ProviderCard 依卡片寬度算好傳進來（10～24 格，utils/layout.ts 的 terminalCellCount）。
  誰會用到：TerminalRow。
-->
<script setup lang="ts">
import { computed } from "vue";
import { filledCellCount } from "../utils/quota";

const props = defineProps<{
  /** 畫面顯示的整數百分比（0～100，超出範圍會被限制） */
  percent: number;
  /** 填滿格子的顏色（漸變色） */
  color: string;
  /** 總格數 */
  cells: number;
}>();

/** 填滿幾格：顯示的整數百分比乘上總格數後四捨五入 */
const filledCount = computed(() => filledCellCount(props.percent, props.cells));

const filledText = computed(() => "█".repeat(filledCount.value));
const emptyText = computed(() => "░".repeat(props.cells - filledCount.value));
</script>

<template>
  <span class="char-bar" aria-hidden="true">
    <span class="char-filled" :style="{ color: color }">{{ filledText }}</span>
    <span class="char-empty">{{ emptyText }}</span>
  </span>
</template>
