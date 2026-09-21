// 檔案：components/settings/form-context.ts
// 用途：設定視窗各頁共用的「欄位錯誤」狀態。由 SettingsView 建立並 provide，
//       欄位元件（SettingField、NumberInput、FieldError）用 useSettingsForm() 取用，
//       這樣不必把錯誤清單一層一層用 props 往下傳。
//       錯誤有兩種來源：Rust 即時檢查的結果（check_settings），以及數字輸入框本身看不懂的文字（例如空白、小數）。
// 誰會用到：SettingsView（提供者）、components/settings/ 底下的欄位元件（使用者）。

import { inject, type InjectionKey } from "vue";

/** 設定表單提供給欄位元件的功能 */
export interface SettingsFormContext {
  /** 某個欄位（例如 "general.refresh_interval_minutes"）要顯示在下方的錯誤文字；沒有錯誤回傳空字串 */
  errorFor(field: string): string;
  /** 數字輸入框回報：這個欄位目前的文字不是整數（true）；恢復正常時傳 false。合法範圍由 SettingsView 依欄位查表、依語言組字 */
  setLocalError(field: string, hasError: boolean): void;
}

/** provide／inject 用的識別鍵 */
export const SETTINGS_FORM_KEY: InjectionKey<SettingsFormContext> = Symbol("settings-form");

/** 欄位元件取得表單狀態；一定要放在 SettingsView 底下使用 */
export function useSettingsForm(): SettingsFormContext {
  const context = inject(SETTINGS_FORM_KEY);
  if (context === undefined) {
    throw new Error("useSettingsForm() 必須在 SettingsView 底下使用");
  }
  return context;
}
