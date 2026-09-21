// 檔案：types/settings.ts
// 用途：設定檔（settings.toml）在畫面端的型別，欄位名稱與 Rust 的 settings/mod.rs 完全一致（41「設定檔」）。
// 誰會用到：services/backend.ts、小工具與設定畫面的元件。

export type ThemeId = "glass" | "minimal" | "terminal" | "gauge" | "neon";
/** 實際套用的色系（auto 已換算成 light 或 dark） */
export type ColorScheme = "light" | "dark";
export type DisplayMode = "carousel" | "overview";
export type ResetTimeFormat = "countdown" | "absolute" | "both";
export type ProviderKey = "claude" | "codex" | "antigravity";

/** 進度條三色與中間色位置（FR-13） */
export interface ColorSettings {
  low: string;
  mid: string;
  high: string;
  mid_position: number;
}

/** 每個「主題＋色系」的卡片邊框：default／none／#RRGGBB（FR-18） */
export interface CardBorderSettings {
  glass_dark: string;
  glass_light: string;
  minimal_dark: string;
  minimal_light: string;
  terminal_dark: string;
  terminal_light: string;
  gauge_dark: string;
  gauge_light: string;
  neon_dark: string;
}

/** 單一服務的設定 */
export interface ProviderSettings {
  enabled: boolean;
  cli_path: string;
  visible_quotas: string[];
}

/** 整份設定 */
export interface Settings {
  schema_version: number;
  general: {
    language: "auto" | "zh-TW" | "en";
    autostart: boolean;
    refresh_interval_minutes: number;
    alert_threshold_percent: number;
    query_timeout_seconds: number;
    log_retention_days: number;
  };
  display: {
    mode: DisplayMode;
    carousel_seconds: number;
    carousel_animation: "slide" | "flip";
    reset_time_format: ResetTimeFormat;
    theme: ThemeId;
    color_scheme: "auto" | ColorScheme;
    opacity_percent: number;
  };
  colors: ColorSettings;
  borders: {
    alert_frame: boolean;
    alert_color: string;
    card: CardBorderSettings;
  };
  providers: Record<ProviderKey, ProviderSettings>;
}

/** 設定警告的種類（和 Rust settings/warning.rs 的 WarningKind 一致） */
export type SettingsWarningKind =
  | "invalid_file"
  | "wrong_type"
  | "out_of_range"
  | "invalid_choice"
  | "invalid_color"
  | "invalid_border"
  | "invalid_cli_path"
  | "unsupported_schema";

/**
 * 一筆設定警告。
 * expected 是給 log 看的中文說明；畫面依語言自己組字，用的是 minimum／maximum（範圍）、choices（選項）、value_type（型別）。
 */
export interface SettingsWarning {
  field: string;
  kind: SettingsWarningKind;
  line: number | null;
  expected: string;
  fallback: string;
  minimum: number | null;
  maximum: number | null;
  choices: string[];
  /** string／integer／boolean／string_list／table */
  value_type: string | null;
}

/** get_settings 指令的回傳：目前設定＋讀檔警告 */
export interface SettingsResponse {
  settings: Settings;
  warnings: SettingsWarning[];
}

/** save_settings 失敗時的內容：欄位警告，或其他錯誤說明 */
export interface SaveSettingsError {
  warnings: SettingsWarning[];
  message: string | null;
}
