//! 檔案：autostart.rs
//! 用途：開機自動啟動的登記與移除（FR-10），全部在 Rust 端處理，畫面端不需要任何開機啟動權限。
//!       - 程式啟動時：設定是開的，就重新登記一次「目前 exe 的位置」（免安裝版搬家後仍有效）。
//!       - 設定儲存後：依新設定登記或移除。
//! 誰會用到：lib.rs（啟動時）、commands.rs 的 save_settings。

use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

/// 依設定登記或移除開機自動啟動；失敗只寫 log，不影響程式其他功能。
pub fn apply(app: &AppHandle, enabled: bool) {
    let launcher = app.autolaunch();

    if enabled {
        match launcher.enable() {
            Ok(()) => log::info!("已登記開機自動啟動（目前 exe 位置）"),
            Err(error) => log::warn!("登記開機自動啟動失敗：{error}"),
        }
        return;
    }

    // 關閉時：本來有登記才移除，避免對不存在的登記做刪除而產生多餘的錯誤訊息
    let is_registered = launcher.is_enabled().unwrap_or(false);
    if !is_registered {
        return;
    }
    match launcher.disable() {
        Ok(()) => log::info!("已移除開機自動啟動"),
        Err(error) => log::warn!("移除開機自動啟動失敗：{error}"),
    }
}
