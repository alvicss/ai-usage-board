// 檔案：services/preview-data.ts
// 用途：在一般瀏覽器用 `npm run dev` 預覽畫面時使用的示範資料（不是真的用量，也不會呼叫任何 CLI）。
//       網址可加參數快速切換外觀，例如 http://localhost:1420/?theme=gauge&scheme=light&mode=overview&lang=en
//       - theme／scheme／mode／lang：主題、色系、顯示方式、語言
//       - only=codex：只顯示某一家（和畫面示意「五種主題，亮暗並列」一樣只看 Codex 卡片）
//       - stage=1：在小工具後面畫一塊示意桌布（亮色時是白天、暗色時是夜晚），玻璃主題才看得出透光
//       - size=360x230：小工具畫在固定大小的框裡（截圖比對用；無頭瀏覽器的視窗最小寬度比小工具大）
//       - opacity=：整體不透明度（30～100）
//       - material=none：模擬沒有 Acrylic 的 Windows 10（玻璃主題改半透明底）
//       設定畫面「外觀」頁上方的即時預覽，也用這裡的示範卡片（previewCardProvider）。
// 誰會用到：services/backend.ts（只在不是 Tauri 程式時使用）、components/settings/WidgetPreview.vue、App.vue（示意桌布）。

import type { AppInfo, RuntimeSummary, SystemInfo } from "../types/app";
import type { Settings, SettingsResponse } from "../types/settings";
import type { ProviderUsage, UsageSnapshot } from "../types/usage";

/** 讀網址參數（單元測試的 Node 環境沒有 window，當作沒有參數） */
function urlParameters(): URLSearchParams {
  const search = typeof window === "undefined" ? "" : window.location.search;
  return new URLSearchParams(search);
}

/** 讀網址裡的整數參數；沒有或看不懂時用預設值 */
function numberParameter(name: string, fallback: number): number {
  const text = urlParameters().get(name);
  if (text === null) {
    return fallback;
  }
  const value = Number(text);
  if (Number.isNaN(value)) {
    return fallback;
  }
  return value;
}

/** 預覽用的電腦環境：語言依瀏覽器語言判斷；預設當作有 Acrylic（網址 material=none 模擬 Windows 10） */
export function previewSystemInfo(): SystemInfo {
  const browserLanguage = typeof navigator === "undefined" ? "" : navigator.language;
  const language = browserLanguage.toLowerCase().startsWith("zh") ? "zh-TW" : "en";
  return { language, acrylic: urlParameters().get("material") !== "none" };
}

/** 網址有 stage=1 時，在小工具後面畫示意桌布（只有瀏覽器預覽用） */
export function previewWantsStage(): boolean {
  return urlParameters().get("stage") === "1";
}

/**
 * 網址有 size=寬x高（例如 size=360x230）時，小工具畫在固定大小的框裡（四周露出示意桌布），
 * 用來截圖比對畫面示意；瀏覽器視窗再大也照這個大小排版。沒有這個參數時回傳 null。
 */
export function previewFrameSize(): { width: number; height: number } | null {
  const text = urlParameters().get("size");
  if (text === null) {
    return null;
  }
  const parts = text.split("x");
  const width = Number(parts[0]);
  const height = Number(parts[1]);
  if (parts.length !== 2 || Number.isNaN(width) || Number.isNaN(height)) {
    return null;
  }
  return { width, height };
}

/** 與 src-tauri/config/default-settings.toml 相同的預設值（預覽用，主題預設 minimal，可用網址參數切換） */
function previewSettings(): Settings {
  const parameters = urlParameters();
  const theme = parameters.get("theme") ?? "minimal";
  const scheme = parameters.get("scheme") ?? "auto";
  const mode = parameters.get("mode") ?? "carousel";
  const language = parameters.get("lang") ?? "auto";

  return {
    schema_version: 1,
    general: {
      language: language as Settings["general"]["language"],
      autostart: false,
      refresh_interval_minutes: 10,
      alert_threshold_percent: 80,
      query_timeout_seconds: 45,
      log_retention_days: 7,
    },
    display: {
      mode: mode === "overview" ? "overview" : "carousel",
      carousel_seconds: 8,
      carousel_animation: "slide",
      reset_time_format: "countdown",
      theme: theme as Settings["display"]["theme"],
      color_scheme: scheme as Settings["display"]["color_scheme"],
      opacity_percent: numberParameter("opacity", 100),
    },
    colors: { low: "#01B468", mid: "#EA7500", high: "#FF0000", mid_position: 60 },
    borders: {
      alert_frame: true,
      alert_color: "#FF3B3B",
      card: {
        glass_dark: "default",
        glass_light: "default",
        minimal_dark: "default",
        minimal_light: "default",
        terminal_dark: "default",
        terminal_light: "default",
        gauge_dark: "default",
        gauge_light: "default",
        neon_dark: "default",
      },
    },
    providers: {
      claude: { enabled: true, cli_path: "", visible_quotas: ["claude.five_hour", "claude.seven_day"] },
      codex: { enabled: true, cli_path: "", visible_quotas: ["codex.primary", "codex.secondary"] },
      antigravity: {
        enabled: true,
        cli_path: "",
        visible_quotas: [
          "antigravity.gemini-5h",
          "antigravity.gemini-weekly",
          "antigravity.3p-5h",
          "antigravity.3p-weekly",
        ],
      },
    },
  };
}

/** 預覽用的設定＋空的警告清單 */
export function previewSettingsResponse(): SettingsResponse {
  return { settings: previewSettings(), warnings: [] };
}

/**
 * 預覽用的用量資料：Claude 正常、Codex 超過警示門檻、Antigravity 離線但有舊資料。
 * Codex 的數字與畫面示意「五種主題，亮暗並列」相同（5 小時 88%、每週 46%），方便截圖比對。
 * 網址有 only=<服務 ID> 時只留那一家。
 */
export function previewSnapshot(): UsageSnapshot {
  const now = Math.floor(Date.now() / 1000);
  const snapshot = previewSnapshotAt(now);
  const only = urlParameters().get("only");
  if (only === null) {
    return snapshot;
  }
  const kept: ProviderUsage[] = [];
  for (const provider of snapshot.providers) {
    if (provider.id === only) {
      kept.push(provider);
    }
  }
  return { ...snapshot, providers: kept };
}

/** 指定「現在時間」的預覽用量資料 */
function previewSnapshotAt(now: number): UsageSnapshot {
  return {
    schema_version: 1,
    updated_at: now,
    providers: [
      {
        id: "claude",
        name: "Claude",
        enabled: true,
        plan: "Max",
        status: "ok",
        status_detail: null,
        last_attempt_at: now - 180,
        last_success_at: now - 180,
        quotas: [
          { id: "claude.five_hour", label_key: "quota.five_hour", label_param: null, used_percent: 62, resets_at: now + 7980, window_minutes: 300 },
          { id: "claude.seven_day", label_key: "quota.weekly", label_param: null, used_percent: 27, resets_at: now + 273600, window_minutes: 10080 },
          { id: "claude.model.fable", label_key: "quota.model_weekly", label_param: "Fable", used_percent: 35, resets_at: now + 273600, window_minutes: 10080 },
        ],
      },
      {
        id: "codex",
        name: "Codex",
        enabled: true,
        plan: "Plus",
        status: "ok",
        status_detail: null,
        last_attempt_at: now - 180,
        last_success_at: now - 180,
        quotas: [
          { id: "codex.primary", label_key: "quota.five_hour", label_param: null, used_percent: 88, resets_at: now + 3930, window_minutes: 300 },
          { id: "codex.secondary", label_key: "quota.weekly", label_param: null, used_percent: 46, resets_at: now + 385230, window_minutes: 10080 },
        ],
      },
      {
        id: "antigravity",
        name: "Antigravity",
        enabled: true,
        plan: null,
        status: "offline",
        status_detail: null,
        last_attempt_at: now - 60,
        last_success_at: now - 2520,
        quotas: [
          { id: "antigravity.gemini-5h", label_key: "quota.five_hour", label_param: "Gemini", used_percent: 16, resets_at: now + 10800, window_minutes: 300 },
          { id: "antigravity.gemini-weekly", label_key: "quota.weekly", label_param: "Gemini", used_percent: 39, resets_at: now + 432000, window_minutes: 10080 },
          { id: "antigravity.3p-5h", label_key: "quota.five_hour", label_param: "Claude+GPT", used_percent: 5, resets_at: now + 9600, window_minutes: 300 },
          { id: "antigravity.3p-weekly", label_key: "quota.weekly", label_param: "Claude+GPT", used_percent: 12, resets_at: now + 500000, window_minutes: 10080 },
        ],
      },
    ],
  };
}

/** 預覽用的關於頁資訊 */
export function previewAppInfo(): AppInfo {
  return { name: "AI Usage Board", version: "v1.0", license: "MIT" };
}

/** 預覽用的程式狀態：Fable 額度還沒看過（設定頁標「新」）；網址加 ?fuse=1 可預覽保險絲跳脫的提示 */
export function previewRuntimeSummary(): RuntimeSummary {
  const tripped = urlParameters().get("fuse") === "1";
  return {
    antigravity_fuse: {
      tripped,
      tripped_at: tripped ? Math.floor(Date.now() / 1000) - 3600 : null,
      num_turns: tripped ? 1 : null,
    },
    seen_quota_ids: [
      "claude.five_hour",
      "claude.seven_day",
      "codex.primary",
      "codex.secondary",
      "antigravity.gemini-5h",
      "antigravity.gemini-weekly",
      "antigravity.3p-5h",
      "antigravity.3p-weekly",
    ],
  };
}

/** 設定畫面預覽用的額度 ID（預覽時一律顯示這兩個，不受使用者的勾選影響） */
export const PREVIEW_CARD_QUOTA_IDS = ["preview.five_hour", "preview.weekly"];

/**
 * 設定畫面「外觀」頁即時預覽用的示範卡片：一個超過預設警示門檻（88%），一個中段（45%），
 * 方便一眼看出進度條配色、警示外框與卡片邊框的效果。不是真的用量。
 */
export function previewCardProvider(nowSeconds: number): ProviderUsage {
  return {
    id: "claude",
    name: "Claude",
    enabled: true,
    plan: "Max",
    status: "ok",
    status_detail: null,
    last_attempt_at: nowSeconds - 180,
    last_success_at: nowSeconds - 180,
    quotas: [
      { id: PREVIEW_CARD_QUOTA_IDS[0], label_key: "quota.five_hour", label_param: null, used_percent: 88, resets_at: nowSeconds + 7980, window_minutes: 300 },
      { id: PREVIEW_CARD_QUOTA_IDS[1], label_key: "quota.weekly", label_param: null, used_percent: 45, resets_at: nowSeconds + 273600, window_minutes: 10080 },
    ],
  };
}
