// 檔案：services/backend.ts
// 用途：畫面呼叫 Rust 指令、監聽 Rust 事件的「唯一」入口（40「Vue 與 Rust 的介面」），元件不直接碰 IPC。
//       在一般瀏覽器用 `npm run dev` 預覽時（沒有 Tauri），改回傳 preview-data.ts 的示範資料，方便看畫面。
// 誰會用到：WidgetView、SettingsView 與設定視窗的各頁元件。

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AppInfo, RuntimeSummary, SystemInfo } from "../types/app";
import type { DisplayMode, ProviderKey, Settings, SettingsResponse, SettingsWarning } from "../types/settings";
import type { UsageSnapshot } from "../types/usage";
import {
  previewAppInfo,
  previewRuntimeSummary,
  previewSettingsResponse,
  previewSnapshot,
  previewSystemInfo,
} from "./preview-data";

/** refresh-state 事件的內容 */
export interface RefreshState {
  running: boolean;
}

/** 設定視窗的分頁 */
export type SettingsTab = "general" | "appearance" | "providers" | "about";

/** 目前是不是在 Tauri 程式裡執行（一般瀏覽器預覽時為 false） */
export const inTauri = "__TAURI_INTERNALS__" in window;

/** 取消監聽用的空函式（瀏覽器預覽時使用） */
const noListener: UnlistenFn = () => undefined;

// ---------------------------------------------------------------------------
// 指令（畫面 → Rust）
// ---------------------------------------------------------------------------

/** 小工具啟動時取目前快取 */
export function getSnapshot(): Promise<UsageSnapshot> {
  if (!inTauri) {
    return Promise.resolve(previewSnapshot());
  }
  return invoke<UsageSnapshot>("get_snapshot");
}

/** ⟳ 按鈕、右鍵「立即更新」；結果走 usage-updated／refresh-state 事件 */
export function refreshNow(): Promise<void> {
  if (!inTauri) {
    return Promise.resolve();
  }
  return invoke<void>("refresh_now");
}

/** 讀取目前設定＋讀檔警告清單 */
export function getSettings(): Promise<SettingsResponse> {
  if (!inTauri) {
    return Promise.resolve(previewSettingsResponse());
  }
  return invoke<SettingsResponse>("get_settings");
}

/** 設定畫面編輯中的即時檢查（只驗證、不寫檔）；回傳不合法欄位清單，沒有問題就是空陣列 */
export function checkSettings(settings: Settings): Promise<SettingsWarning[]> {
  if (!inTauri) {
    return Promise.resolve([]);
  }
  return invoke<SettingsWarning[]>("check_settings", { settings });
}

/** 設定畫面按儲存；失敗時 Promise 會被拒絕，拒絕的內容是 SaveSettingsError（欄位警告或錯誤說明） */
export function saveSettings(settings: Settings): Promise<Settings> {
  if (!inTauri) {
    return Promise.resolve(settings);
  }
  return invoke<Settings>("save_settings", { settings });
}

/** ⇄ 按鈕：切換輪播／並排（同時寫回設定） */
export function setDisplayMode(mode: DisplayMode): Promise<void> {
  if (!inTauri) {
    return Promise.resolve();
  }
  return invoke<void>("set_display_mode", { mode });
}

/**
 * ⚙ 按鈕、右鍵「設定…」「關於…」：開啟設定視窗。
 * 不指定分頁：已開著時只帶到前面（保留正在看的頁面與未儲存的修改）；指定分頁（例如 about）時切到那一頁。
 */
export function openSettings(tab?: SettingsTab): Promise<void> {
  if (!inTauri) {
    return Promise.resolve();
  }
  return invoke<void>("open_settings", { tab: tab ?? null });
}

/** 設定畫面的「取消」：關閉設定視窗 */
export function closeSettings(): Promise<void> {
  if (!inTauri) {
    return Promise.resolve();
  }
  return invoke<void>("close_settings");
}

/** 右鍵「結束」 */
export function quitApp(): Promise<void> {
  if (!inTauri) {
    return Promise.resolve();
  }
  return invoke<void>("quit_app");
}

/** 小工具拖曳或縮放結束：請 Rust 記住目前顯示方式的位置與大小（位置大小由 Rust 直接讀視窗） */
export function saveWindowBounds(): Promise<void> {
  if (!inTauri) {
    return Promise.resolve();
  }
  return invoke<void>("save_window_bounds");
}

/** 設定頁：某一家 CLI 的自動偵測結果（完整路徑）；找不到回傳 null。只看檔案位置，不執行 CLI */
export function detectCli(provider: ProviderKey): Promise<string | null> {
  if (!inTauri) {
    const previewNames: Record<ProviderKey, string> = { claude: "claude.exe", codex: "codex.cmd", antigravity: "agy.exe" };
    return Promise.resolve(previewNames[provider]);
  }
  return invoke<string | null>("detect_cli", { provider });
}

/** 設定頁：Antigravity 保險絲狀態與看過的額度 ID */
export function getRuntimeState(): Promise<RuntimeSummary> {
  if (!inTauri) {
    return Promise.resolve(previewRuntimeSummary());
  }
  return invoke<RuntimeSummary>("get_runtime_state");
}

/** 設定頁「我已確認，恢復查詢」：重設 Antigravity 保險絲並馬上查一次 */
export function resetAntigravityFuse(): Promise<void> {
  if (!inTauri) {
    return Promise.resolve();
  }
  return invoke<void>("reset_antigravity_fuse");
}

/** 關於頁：程式名稱、兩碼版本、授權 */
export function getAppInfo(): Promise<AppInfo> {
  if (!inTauri) {
    return Promise.resolve(previewAppInfo());
  }
  return invoke<AppInfo>("get_app_info");
}

/** 關於頁點作者：由 Rust 請 Windows 預設瀏覽器開作者網站（網址寫在 Rust 端，畫面不能指定網址） */
export function openHomepage(): Promise<void> {
  if (!inTauri) {
    return Promise.resolve();
  }
  return invoke<void>("open_homepage");
}

/** 關於頁點 GitHub：由 Rust 請 Windows 預設瀏覽器開本專案的 GitHub 頁面（網址寫在 Rust 端，畫面不能指定網址） */
export function openRepository(): Promise<void> {
  if (!inTauri) {
    return Promise.resolve();
  }
  return invoke<void>("open_repository");
}

/** 這台電腦的環境：語言「自動」的結果、玻璃主題能不能用 Acrylic（小工具與設定視窗啟動時各讀一次） */
export function getSystemInfo(): Promise<SystemInfo> {
  if (!inTauri) {
    return Promise.resolve(previewSystemInfo());
  }
  return invoke<SystemInfo>("get_system_info");
}

// ---------------------------------------------------------------------------
// 事件（Rust → 畫面）
// ---------------------------------------------------------------------------

/** usage-updated：每輪查詢完成（含部分失敗） */
export function onUsageUpdated(handler: (snapshot: UsageSnapshot) => void): Promise<UnlistenFn> {
  if (!inTauri) {
    return Promise.resolve(noListener);
  }
  return listen<UsageSnapshot>("usage-updated", (event) => handler(event.payload));
}

/** refresh-state：查詢開始與結束 */
export function onRefreshState(handler: (state: RefreshState) => void): Promise<UnlistenFn> {
  if (!inTauri) {
    return Promise.resolve(noListener);
  }
  return listen<RefreshState>("refresh-state", (event) => handler(event.payload));
}

/** widget-flash：已經在執行時又開了一次，小工具閃一下提示位置（只有小工具在聽） */
export function onWidgetFlash(handler: () => void): Promise<UnlistenFn> {
  if (!inTauri) {
    return Promise.resolve(noListener);
  }
  return listen("widget-flash", () => handler());
}

/** settings-open-tab：設定視窗已開著時按右鍵「關於…」，切到指定分頁（只有設定視窗在聽） */
export function onSettingsOpenTab(handler: (tab: SettingsTab) => void): Promise<UnlistenFn> {
  if (!inTauri) {
    return Promise.resolve(noListener);
  }
  return listen<SettingsTab>("settings-open-tab", (event) => handler(event.payload));
}

/** settings-changed：設定儲存成功、或 ⇄ 切換顯示方式之後 */
export function onSettingsChanged(handler: (settings: Settings) => void): Promise<UnlistenFn> {
  if (!inTauri) {
    return Promise.resolve(noListener);
  }
  return listen<Settings>("settings-changed", (event) => handler(event.payload));
}
