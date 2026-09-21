// 檔案：composables/useRollingPercent.ts
// 用途：百分比數字的「滾動」效果（43「動畫」：查詢完成時從舊值數到新值，600ms，先快後慢）。
//       Windows 關閉動畫效果（prefers-reduced-motion）時直接跳到新值。
//       橫條、字元條、環狀表三種額度顯示都用這一份，行為才會一致。
// 誰會用到：QuotaRow、TerminalRow、GaugeRing。

import { onBeforeUnmount, ref, watch, type Ref } from "vue";
import { displayPercent } from "../utils/quota";

/** 數字滾動的長度（43「動畫」：600ms） */
const ROLL_DURATION_MS = 600;

/**
 * 回傳畫面要顯示的整數百分比；原始值改變時，從舊值滾動到新值。
 * 輸入：取得「已用 %」的函式（例如 () => props.quota.used_percent）。
 */
export function useRollingPercent(source: () => number): Ref<number> {
  const shownPercent = ref(displayPercent(source()));
  let animationFrame = 0;

  watch(source, (newValue) => {
    const target = displayPercent(newValue);
    const start = shownPercent.value;
    cancelAnimationFrame(animationFrame);

    // Windows 關閉動畫效果時直接跳到新值
    const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduceMotion || start === target) {
      shownPercent.value = target;
      return;
    }

    const startTime = performance.now();
    const step = (time: number) => {
      const progress = Math.min(1, (time - startTime) / ROLL_DURATION_MS);
      const eased = 1 - Math.pow(1 - progress, 3); // 先快後慢
      shownPercent.value = Math.round(start + (target - start) * eased);
      if (progress < 1) {
        animationFrame = requestAnimationFrame(step);
      }
    };
    animationFrame = requestAnimationFrame(step);
  });

  onBeforeUnmount(() => cancelAnimationFrame(animationFrame));
  return shownPercent;
}
