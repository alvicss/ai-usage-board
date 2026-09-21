// 檔案：utils/quota.ts
// 用途：額度相關的純判斷：哪些額度要顯示（FR-12 勾選）、畫面顯示的整數百分比與字元條格數（FR-5）、
//       是否超過警示門檻（FR-7）、卡片是否算「查詢失敗」。
// 誰會用到：ProviderCard、CarouselLayout（輪播圓點的警示紅點）、QuotaRow／TerminalRow／GaugeRing／CharBar。

import type { ProviderSettings, Settings } from "../types/settings";
import type { ProviderUsage, Quota } from "../types/usage";

/** 依設定的勾選清單，挑出要顯示的額度（保持 CLI 回報的順序） */
export function visibleQuotas(provider: ProviderUsage, providerSettings: ProviderSettings | undefined): Quota[] {
  if (providerSettings === undefined) {
    return [];
  }
  const shown: Quota[] = [];
  for (const quota of provider.quotas) {
    if (providerSettings.visible_quotas.includes(quota.id)) {
      shown.push(quota);
    }
  }
  return shown;
}

/**
 * 畫面顯示的百分比：四捨五入到整數，並限制在 0～100（例：0.06 → 0、0.5 → 1、99.6 → 100）。
 * 數字、進度條長度、字元條格數、環狀表的弧長、漸變顏色、警示門檻全部用這個整數，
 * 畫面上看到 0% 就完全沒有填色，讓進度條和顯示數字一致。
 */
export function displayPercent(usedPercent: number): number {
  const rounded = Math.round(usedPercent);
  if (rounded < 0) {
    return 0;
  }
  if (rounded > 100) {
    return 100;
  }
  return rounded;
}

/**
 * 工程師主題字元條要填滿幾格：顯示的整數百分比乘上總格數後四捨五入（0% 一定是 0 格）。
 * 輸入：displayPercent 算好的整數百分比、總格數。
 */
export function filledCellCount(shownPercent: number, cells: number): number {
  let percent = shownPercent;
  if (percent < 0) {
    percent = 0;
  }
  if (percent > 100) {
    percent = 100;
  }
  return Math.round((percent / 100) * cells);
}

/** 這一家目前是否處於「查詢失敗」狀態（ok 與「查詢中」都不算） */
export function isFailedStatus(provider: ProviderUsage): boolean {
  return provider.status !== "ok" && provider.status !== "pending" && provider.status !== "disabled";
}

/** 額度是否達到警示門檻；用顯示出來的整數判斷，畫面上看到 80% 就一定會亮警示 */
export function isQuotaAlert(quota: Quota, threshold: number): boolean {
  return displayPercent(quota.used_percent) >= threshold;
}

/**
 * 這一家是否要顯示警示（卡片呼吸外框、輪播紅點）。
 * 只看勾選顯示的額度；查詢失敗（數字已淡化成灰色）時不亮警示，與畫面示意一致。
 */
export function providerHasAlert(provider: ProviderUsage, settings: Settings): boolean {
  if (isFailedStatus(provider)) {
    return false;
  }
  const providerSettings = settings.providers[provider.id];
  const threshold = settings.general.alert_threshold_percent;
  for (const quota of visibleQuotas(provider, providerSettings)) {
    if (isQuotaAlert(quota, threshold)) {
      return true;
    }
  }
  return false;
}
