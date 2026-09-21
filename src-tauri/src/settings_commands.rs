//! 檔案：settings_commands.rs
//! 用途：設定視窗使用的 Rust 指令（40「Vue 與 Rust 的介面」）：
//!       讀取設定、編輯中即時檢查、儲存並立即套用、自動偵測 CLI 位置、讀取保險絲與看過的額度、恢復保險絲。
//!       畫面端只能透過 src/services/backend.ts 呼叫這些指令。
//! 誰會用到：lib.rs 註冊（generate_handler!）；畫面的 backend.ts。

use crate::providers::{self, ProviderId};
use crate::scheduler::{RuntimeSummary, Scheduler};
use crate::settings::{
    SaveSettingsError, Settings, SettingsChanges, SettingsResponse, SettingsStore, SettingsWarning,
};
use crate::{autostart, logging, settings_window, tray, widget_window};
use tauri::{AppHandle, Emitter, State};

/// 讀取目前設定＋讀檔警告清單。
#[tauri::command]
pub fn get_settings(store: State<'_, SettingsStore>) -> SettingsResponse {
    store.response()
}

/// 設定畫面編輯中的即時檢查：只驗證、不寫檔；回傳不合法欄位的清單，畫面顯示在欄位下方。
/// 驗證規則和儲存時完全相同（settings/validate.rs），畫面端不另寫一套。
#[tauri::command]
pub fn check_settings(settings: Settings, store: State<'_, SettingsStore>) -> Vec<SettingsWarning> {
    store.check(settings)
}

/// 設定畫面按「儲存」：驗證 → 備份 settings.toml.bak → 保留註解寫回 → 立即套用 → 通知所有視窗（settings-changed）。
/// 寫成 async：套用時可能要縮放小工具、切換玻璃材質、改設定視窗標題列，放在背景執行緒發出，避免在畫面事件處理途中改視窗。
#[tauri::command]
pub async fn save_settings(
    settings: Settings,
    app: AppHandle,
    store: State<'_, SettingsStore>,
    scheduler: State<'_, Scheduler>,
) -> Result<Settings, SaveSettingsError> {
    let before = store.current();
    let saved = store.save(settings)?;
    apply_saved_settings(&app, &scheduler, &before, &saved);
    Ok(saved)
}

/// 儲存成功後，把新設定立即套用到程式各處（FR-9：不用重啟）。
fn apply_saved_settings(
    app: &AppHandle,
    scheduler: &Scheduler,
    before: &Settings,
    saved: &Settings,
) {
    let changes = SettingsChanges::between(before, saved);

    // 1. 開機自動啟動：打開就登記「目前 exe 位置」、關閉就移除（FR-10）
    autostart::apply(app, saved.general.autostart);

    // 2. log 保留天數：更新並馬上清掉超過新天數的舊檔
    if changes.log_retention {
        logging::set_retention_days(saved.general.log_retention_days);
    }

    // 3. 更新間隔：叫醒排程，立刻用新間隔重新判斷（不必等滿 30 秒）
    if changes.refresh_interval {
        scheduler.wake();
    }

    // 4. 顯示方式在設定畫面被改了：換成該模式記住的視窗大小（和 ⇄ 按鈕相同）
    if changes.display_mode {
        if let Err(error) = widget_window::apply_mode_size(app, scheduler, &saved.display.mode) {
            log::warn!("切換顯示方式時調整視窗大小失敗：{error}");
        }
    }

    // 5. 剛啟用某一家、或換了 CLI 位置：馬上查一次，讓使用者看到結果
    if changes.needs_requery {
        scheduler.request_refresh(app.clone(), "服務設定變更");
    }

    // 6. 設定頁列出的額度使用者都看過了：「新」標記到此為止
    scheduler.mark_all_quotas_seen();

    // 7. 小工具視窗的亮暗與玻璃材質跟著新的主題、色系（43「玻璃材質」）
    widget_window::apply_appearance(app, saved);

    // 8. 語言改了：系統匣選單換成新語言（小工具右鍵選單由畫面收到 settings-changed 後自己重建）
    if changes.language {
        tray::apply_language(app, &saved.general.language);
    }

    // 9. 設定視窗標題列的亮暗跟著新色系；語言、主題、邊框等由畫面收到 settings-changed 後自己套用
    settings_window::apply_theme(app, &saved.display.color_scheme);
    let _ = app.emit("settings-changed", saved.clone());
}

/// 設定頁「CLI 位置」旁顯示的自動偵測結果：找到的完整路徑；找不到回傳 None。
/// 只看檔案位置，不執行任何 CLI。
#[tauri::command]
pub fn detect_cli(provider: ProviderId) -> Option<String> {
    let path = providers::auto_detect_cli(provider)?;
    Some(path.to_string_lossy().into_owned())
}

/// 設定頁需要的狀態：Antigravity 保險絲、看過的額度（標「新」用）。
#[tauri::command]
pub fn get_runtime_state(scheduler: State<'_, Scheduler>) -> RuntimeSummary {
    scheduler.runtime_summary()
}

/// 設定頁「我已確認，恢復查詢」：重設 Antigravity 保險絲並馬上查一次。
#[tauri::command]
pub fn reset_antigravity_fuse(
    app: AppHandle,
    scheduler: State<'_, Scheduler>,
) -> Result<(), String> {
    scheduler.reset_antigravity_fuse()?;
    scheduler.request_refresh(app, "Antigravity 保險絲恢復");
    Ok(())
}
