//! 檔案：settings/store.rs
//! 用途：保存「程式執行中目前使用的設定」與讀檔警告，並提供儲存、切換顯示方式等操作。
//!       Tauri 以 managed state 的方式共用這一份（畫面指令與背景排程都從這裡讀設定）。
//! 誰會用到：lib.rs（啟動時建立）、settings_commands.rs（get／check／save_settings）、commands.rs（set_display_mode）、
//!           scheduler（每輪查詢前讀取最新設定）。

use super::load::{load_or_create, LoadedSettings};
use super::validate::validate_settings;
use super::writer::{write_display_mode, write_settings};
use super::{default_settings, Settings, SettingsError, SettingsWarning};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// get_settings 指令的回傳內容：目前設定＋讀檔警告清單（40「Vue 與 Rust 的介面」）。
#[derive(Debug, Clone, Serialize)]
pub struct SettingsResponse {
    pub settings: Settings,
    pub warnings: Vec<SettingsWarning>,
}

/// save_settings 失敗時回傳給設定畫面的內容。
#[derive(Debug, Clone, Serialize)]
pub struct SaveSettingsError {
    /// 不合法的欄位（設定畫面標在欄位旁）
    pub warnings: Vec<SettingsWarning>,
    /// 其他錯誤（例如無法寫檔）的簡短說明；只有欄位錯誤時為 None
    pub message: Option<String>,
}

/// 執行中的設定狀態。
pub struct SettingsStore {
    path: PathBuf,
    state: Mutex<LoadedSettings>,
}

impl SettingsStore {
    /// 程式啟動時讀取 settings.toml（沒有就建立）。
    pub fn load(settings_path: &Path) -> Result<Self, SettingsError> {
        let loaded = load_or_create(settings_path)?;
        Ok(Self {
            path: settings_path.to_path_buf(),
            state: Mutex::new(loaded),
        })
    }

    /// 取得目前設定的複本。
    pub fn current(&self) -> Settings {
        match self.state.lock() {
            Ok(state) => state.settings.clone(),
            // 鎖壞掉（只會在其他執行緒 panic 後發生）時，仍取出裡面的值，避免程式當掉
            Err(poisoned) => poisoned.into_inner().settings.clone(),
        }
    }

    /// 取得目前設定＋讀檔警告。
    pub fn response(&self) -> SettingsResponse {
        let state = match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        SettingsResponse {
            settings: state.settings.clone(),
            warnings: state.warnings.clone(),
        }
    }

    /// 讀檔時的警告（lib.rs 啟動時寫進 log 用）。
    pub fn load_warnings(&self) -> Vec<SettingsWarning> {
        self.response().warnings
    }

    /// 設定畫面編輯中的即時檢查：只驗證、不寫檔，回傳不合法欄位的清單（沒有問題就是空清單）。
    pub fn check(&self, candidate: Settings) -> Vec<SettingsWarning> {
        match default_settings() {
            Ok(defaults) => validate_settings(candidate, &defaults).warnings,
            Err(error) => {
                log::error!("{error}");
                Vec::new()
            }
        }
    }

    /// 設定畫面按「儲存」：驗證 → 有任何不合法欄位就整批退回；全部合法才備份並寫檔。
    /// 成功時回傳實際儲存的設定；失敗時回傳欄位警告清單（設定畫面標在欄位旁）或錯誤說明。
    pub fn save(&self, new_settings: Settings) -> Result<Settings, SaveSettingsError> {
        let defaults = match default_settings() {
            Ok(defaults) => defaults,
            Err(error) => {
                log::error!("{error}");
                return Err(SaveSettingsError {
                    warnings: Vec::new(),
                    message: Some(error.to_string()),
                });
            }
        };
        let validated = validate_settings(new_settings, &defaults);
        if !validated.warnings.is_empty() {
            return Err(SaveSettingsError {
                warnings: validated.warnings,
                message: None,
            });
        }

        if let Err(error) = write_settings(&self.path, &validated.settings) {
            log::error!("寫入 settings.toml 失敗：{error}");
            return Err(SaveSettingsError {
                warnings: Vec::new(),
                message: Some(error.to_string()),
            });
        }

        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.settings = validated.settings.clone();
        // 使用者已經存過一次完整設定，原本的讀檔警告不再適用
        state.warnings.clear();
        state.file_is_broken = false;
        log::info!("設定已儲存");
        Ok(validated.settings)
    }

    /// ⇄ 按鈕：切換顯示方式並寫回設定檔。
    /// 設定檔整份壞掉時只改記憶體中的值，不覆寫原檔（FR-9：按儲存之前不覆寫）。
    pub fn set_display_mode(&self, mode: &str) -> Result<Settings, String> {
        if mode != "carousel" && mode != "overview" {
            return Err("顯示方式只接受 carousel 或 overview".to_string());
        }

        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        state.settings.display.mode = mode.to_string();

        if state.file_is_broken {
            log::warn!("settings.toml 格式錯誤，顯示方式只在本次執行生效，未寫回檔案");
            return Ok(state.settings.clone());
        }

        match write_display_mode(&self.path, mode) {
            Ok(true) => {}
            Ok(false) => log::warn!("settings.toml 目前無法解析，顯示方式未寫回檔案"),
            Err(error) => log::error!("寫入顯示方式失敗：{error}"),
        }
        Ok(state.settings.clone())
    }
}
