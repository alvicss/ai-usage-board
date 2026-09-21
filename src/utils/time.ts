// 檔案：utils/time.ts
// 用途：時間相關的純計算（不含任何顯示文字，文字由 i18n/format.ts 組合）：
//       重置倒數拆成天／時／分、絕對時間 "09/20 14:00"、距今幾分鐘。
// 誰會用到：i18n/format.ts。

/** 倒數的組成：已過期，或剩幾天幾時幾分 */
export interface CountdownParts {
  expired: boolean;
  days: number;
  hours: number;
  minutes: number;
}

/**
 * 算出距離重置還剩多久。
 * 輸入：重置時間（epoch 秒）、現在時間（毫秒）。
 * 分鐘數無條件進位：還剩 30 秒時顯示「1 分後」，不會出現「0 分後」。
 */
export function countdownParts(resetsAtSeconds: number, nowMilliseconds: number): CountdownParts {
  const remainingMilliseconds = resetsAtSeconds * 1000 - nowMilliseconds;
  if (remainingMilliseconds <= 0) {
    return { expired: true, days: 0, hours: 0, minutes: 0 };
  }

  const totalMinutes = Math.ceil(remainingMilliseconds / 60_000);
  const days = Math.floor(totalMinutes / 1440);
  const hours = Math.floor((totalMinutes % 1440) / 60);
  const minutes = totalMinutes % 60;
  return { expired: false, days, hours, minutes };
}

/** 補零成兩位數，例如 9 → "09" */
function twoDigits(value: number): string {
  return String(value).padStart(2, "0");
}

/** 絕對時間（本機時區、24 小時制），例如 "09/20 14:00"（FR-5） */
export function formatAbsolute(epochSeconds: number): string {
  const date = new Date(epochSeconds * 1000);
  const month = twoDigits(date.getMonth() + 1);
  const day = twoDigits(date.getDate());
  const hour = twoDigits(date.getHours());
  const minute = twoDigits(date.getMinutes());
  return `${month}/${day} ${hour}:${minute}`;
}

/** 距今幾分鐘（無條件捨去）；沒有時間時回傳 null */
export function minutesSince(epochSeconds: number | null, nowMilliseconds: number): number | null {
  if (epochSeconds === null) {
    return null;
  }
  const elapsed = nowMilliseconds - epochSeconds * 1000;
  if (elapsed <= 0) {
    return 0;
  }
  return Math.floor(elapsed / 60_000);
}
