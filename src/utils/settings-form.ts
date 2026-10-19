// 檔案：utils/settings-form.ts
// 用途：設定畫面用到的純計算（不碰畫面、不呼叫 Rust，可以直接寫單元測試）：
//       1. 數字欄位的範圍：給輸入框、滑桿當上下限、組錯誤訊息；最後的檢查以 Rust 的 settings/validate.rs 為準。
//       2. 「還原預設」用的預設值（與 src-tauri/config/default-settings.toml 一致，單元測試會比對）。
//       3. 卡片邊框（FR-18）：目前主題＋色系對應哪個欄位、目前是哪種模式、怎麼組回設定值。
//       4. 欄位錯誤：從 Rust 回傳的警告清單找出某欄位的警告。
//       5. 整數輸入的解析。
// 誰會用到：設定視窗的元件（SettingsView 與 components/settings/ 底下各頁）。

import type { CardBorderSettings, ColorScheme, ColorSettings, SettingsWarning, ThemeId } from "../types/settings";
import { isHexColor, type BorderMode } from "./appearance";

/**
 * 數字欄位的合法範圍（與 41「設定檔」註解、Rust validate.rs 相同）。
 * 給輸入框、滑桿當上下限；輸入框裡不是整數時，錯誤訊息也從這裡取範圍（文字由翻譯檔依語言組成）。
 */
export const NUMBER_RANGES = {
  "general.refresh_interval_minutes": { min: 1, max: 120 },
  "general.alert_threshold_percent": { min: 50, max: 99 },
  "general.query_timeout_seconds": { min: 10, max: 120 },
  "general.log_retention_days": { min: 1, max: 90 },
  "display.carousel_seconds": { min: 3, max: 60 },
  "display.opacity_percent": { min: 30, max: 100 },
  "colors.mid_position": { min: 1, max: 99 },
} as const;

/** 有範圍限制的數字欄位名稱 */
export type NumberField = keyof typeof NUMBER_RANGES;

/** 進度條配色的預設值（FR-13） */
export const DEFAULT_COLORS: ColorSettings = {
  low: "#01B468",
  mid: "#EA7500",
  high: "#FF0000",
  mid_position: 60,
};

/** 警示外框的預設值（FR-18） */
export const DEFAULT_ALERT_FRAME = true;
export const DEFAULT_ALERT_COLOR = "#FF3B3B";

/** 卡片邊框從「主題預設」切到「自訂顏色」時，顏色選擇器一開始的顏色 */
export const DEFAULT_CUSTOM_BORDER_COLOR = "#5B8DEF";

/** 目前主題＋色系對應 [borders.card] 的哪一個欄位；霓虹只有暗色，所以固定 neon_dark */
export function borderFieldKey(theme: ThemeId, scheme: ColorScheme): keyof CardBorderSettings {
  if (theme === "neon") {
    return "neon_dark";
  }
  return `${theme}_${scheme}` as keyof CardBorderSettings;
}

/** 邊框設定值是哪一種模式：default（主題預設）、none（無邊框）、其他 #RRGGBB（自訂顏色） */
export function borderModeOf(value: string): BorderMode {
  if (value === "none") {
    return "none";
  }
  if (isHexColor(value)) {
    return "custom";
  }
  return "default";
}

/** 依選到的模式組回要寫進設定的值；自訂顏色時用 customColor */
export function borderValueFor(mode: BorderMode, customColor: string): string {
  if (mode === "none") {
    return "none";
  }
  if (mode === "custom") {
    return customColor;
  }
  return "default";
}

/** 找出某個欄位的警告（例如 "general.refresh_interval_minutes"）；沒有就回傳 undefined */
export function warningFor(field: string, warnings: SettingsWarning[]): SettingsWarning | undefined {
  for (const warning of warnings) {
    if (warning.field === field) {
      return warning;
    }
  }
  return undefined;
}

/**
 * 把輸入框的文字轉成整數；空白、小數、夾雜文字都回傳 null（畫面會提示「請填整數」）。
 * 前後空白可以接受。
 */
export function parseInteger(text: string): number | null {
  const trimmed = text.trim();
  if (!/^-?\d+$/.test(trimmed)) {
    return null;
  }
  return Number(trimmed);
}
