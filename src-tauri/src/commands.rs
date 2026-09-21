//! 檔案：commands.rs
//! 用途：小工具視窗使用的 Rust 指令（40「Vue 與 Rust 的介面」）：取用量快取、立即更新、切換顯示方式、
//!       記住視窗位置大小、開關設定視窗、結束程式。設定視窗專用的指令在 settings_commands.rs，
//!       關於頁的指令在 about.rs。一律列在 40 的表，不另開其他後門。
//!       畫面端只能透過 src/services/backend.ts 呼叫這些指令。
//! 誰會用到：lib.rs 註冊（generate_handler!）；畫面的 backend.ts。

use crate::cache::UsageSnapshot;
use crate::scheduler::Scheduler;
use crate::settings::SettingsStore;
use crate::{settings_window, widget_window};
use tauri::{AppHandle, Emitter, State};

/// 小工具啟動時取得目前快取。
#[tauri::command]
pub fn get_snapshot(scheduler: State<'_, Scheduler>) -> UsageSnapshot {
    scheduler.snapshot()
}

/// ⟳ 按鈕、右鍵「立即更新」：要求查詢一輪；結果走 usage-updated／refresh-state 事件。
/// 查詢進行中時不另起查詢（FR-3）。
#[tauri::command]
pub fn refresh_now(app: AppHandle, scheduler: State<'_, Scheduler>) {
    scheduler.request_refresh(app, "手動更新");
}

/// ⇄ 按鈕：切換輪播／並排，同時寫回設定；視窗換成該模式記住的大小（以右上角為基準），並通知所有視窗。
/// 寫成 async：會移動／縮放視窗，放在背景執行緒發出，由主執行緒依序處理，避免在畫面事件處理途中改視窗。
#[tauri::command]
pub async fn set_display_mode(
    mode: String,
    app: AppHandle,
    store: State<'_, SettingsStore>,
    scheduler: State<'_, Scheduler>,
) -> Result<(), String> {
    change_display_mode(&app, &store, &scheduler, &mode)
}

/// 切換顯示方式的共同步驟（⇄ 按鈕與系統匣選單都用這個）：寫回設定 → 換成該模式記住的大小 → 通知所有視窗。
/// 會移動、縮放視窗，呼叫端要在背景執行緒（async 指令或另開的執行緒），不要在主執行緒處理視窗訊息的途中呼叫。
pub fn change_display_mode(
    app: &AppHandle,
    store: &SettingsStore,
    scheduler: &Scheduler,
    mode: &str,
) -> Result<(), String> {
    let settings = store.set_display_mode(mode)?;
    if let Err(error) = widget_window::apply_mode_size(app, scheduler, mode) {
        log::warn!("切換顯示方式時調整視窗大小失敗：{error}");
    }
    let _ = app.emit("settings-changed", settings);
    Ok(())
}

/// 小工具拖曳或縮放結束：記住「目前顯示方式」的位置與大小（M-005）。
/// 位置大小由 Rust 直接讀視窗，顯示方式由 Rust 讀目前設定，畫面只負責通知「動完了」。
#[tauri::command]
pub async fn save_window_bounds(
    app: AppHandle,
    store: State<'_, SettingsStore>,
    scheduler: State<'_, Scheduler>,
) -> Result<(), String> {
    let mode = store.current().display.mode;
    widget_window::remember_current_bounds(&app, &scheduler, &mode)
}

/// ⚙ 按鈕、右鍵「設定…」「關於…」：開啟設定視窗（tab 可省略；已開著時只帶到前面，有指定分頁才切換）。
/// 必須是 async：Windows 上在同步指令裡建立視窗可能卡死（Tauri 官方說明）。
#[tauri::command]
pub async fn open_settings(
    app: AppHandle,
    tab: Option<String>,
    store: State<'_, SettingsStore>,
) -> Result<(), String> {
    let color_scheme = store.current().display.color_scheme;
    settings_window::open(&app, tab.as_deref(), &color_scheme)
}

/// 設定畫面的「取消」：不寫檔，直接關閉設定視窗。
/// 寫成 async：關閉視窗改在背景執行緒發出、由主執行緒依序處理，不在畫面事件處理途中直接關視窗（22 踩過的坑）。
#[tauri::command]
pub async fn close_settings(app: AppHandle) -> Result<(), String> {
    settings_window::close(&app)
}

/// 右鍵「結束」（系統匣的「結束」在 tray.rs，做的事相同）。同樣寫成 async，結束程式的要求由主執行緒依序處理。
/// 結束時查詢中的 CLI 子行程由 Job Object 一起收掉，系統匣圖示由 Tauri 移除。
#[tauri::command]
pub async fn quit_app(app: AppHandle) {
    log::info!("使用者結束程式");
    app.exit(0);
}
