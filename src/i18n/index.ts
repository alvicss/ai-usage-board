// 檔案：i18n/index.ts
// 用途：建立繁中與英文翻譯入口；畫面元件的文字一律從翻譯檔取得，不直接寫在元件裡。
// 誰會用到：main.ts（掛到 Vue 上）、WidgetView 與 SettingsView（切換語言）。

import { createI18n } from "vue-i18n";
import zhTW from "./zh-TW.json";
import en from "./en.json";

export type AppLocale = "zh-TW" | "en";

export const i18n = createI18n({
  legacy: false,
  locale: "zh-TW",
  fallbackLocale: "zh-TW",
  messages: { "zh-TW": zhTW, en },
});

/**
 * 依設定的語言決定實際使用的語系（FR-17）。
 * 輸入：設定的語言（auto／zh-TW／en）、「自動」時的語言。
 * 「自動」時的語言由 Rust 依 Windows 顯示語言與地區判斷好（system_info.rs：顯示語言是中文，
 * 或地區是台灣、香港、澳門 → 繁中，其他 → 英文），這裡只要不是中文開頭就用英文。
 * 手動選的語言優先。
 */
export function resolveLocale(language: string, systemLanguage: string): AppLocale {
  if (language === "zh-TW" || language === "en") {
    return language;
  }
  if (systemLanguage.toLowerCase().startsWith("zh")) {
    return "zh-TW";
  }
  return "en";
}
