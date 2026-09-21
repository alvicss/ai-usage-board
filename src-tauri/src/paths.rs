//! 檔案：paths.rs
//! 用途：集中建立與提供 %APPDATA%\AI Usage Board\ 底下的程式資料路徑（41「檔案位置」）。
//!       位置由系統 API 取得（Roaming 設定資料夾），不寫死使用者名稱或磁碟代號。
//! 誰會用到：lib.rs 啟動流程；設定、log、快取、狀態檔模組。

use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// 程式資料夾名稱（放在 Roaming 設定資料夾底下）。
const APP_DIRECTORY_NAME: &str = "AI Usage Board";

/// 程式會用到的所有檔案與資料夾位置。
#[derive(Debug)]
pub struct AppDirectories {
    pub logs: PathBuf,
    pub cache: PathBuf,
    pub state: PathBuf,
    pub settings: PathBuf,
}

impl AppDirectories {
    /// 取得 Windows Roaming 設定資料夾，並建立本程式會用到的子資料夾。
    pub fn prepare(app_handle: &AppHandle) -> Result<Self, std::io::Error> {
        let roaming_config_directory = app_handle
            .path()
            .config_dir()
            .map_err(std::io::Error::other)?;

        let root = roaming_config_directory.join(APP_DIRECTORY_NAME);
        let logs = root.join("logs");

        fs::create_dir_all(&logs)?;

        Ok(Self {
            cache: root.join("cache.json"),
            state: root.join("state.json"),
            settings: root.join(crate::settings::SETTINGS_FILE_NAME),
            logs,
        })
    }
}
