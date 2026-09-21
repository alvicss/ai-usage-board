//! 檔案：settings/mod.rs
//! 用途：定義 settings.toml 的資料結構（欄位與 41_DATA_MODEL.md「設定檔」一節一一對應），
//!       並把讀取、驗證、寫回、執行中狀態四個子模組集中對外公開。
//! 誰會用到：lib.rs 啟動流程、commands.rs、scheduler。
//!
//! 子模組分工：
//! - load.rs：讀檔、和內建預設值合併、型別檢查（壞檔時安全改用預設值）
//! - validate.rs／checks.rs：每個欄位的範圍與格式檢查
//! - writer.rs：用 toml_edit 只改值、保留註解寫回
//! - store.rs：程式執行中「目前的設定」與讀檔警告
//! - changes.rs：比較儲存前後的設定，決定哪些要立即套用

mod changes;
mod checks;
mod load;
mod store;
mod validate;
mod warning;
mod writer;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

pub use changes::SettingsChanges;
pub use store::{SaveSettingsError, SettingsResponse, SettingsStore};
pub use warning::SettingsWarning;

/// 程式內建的預設設定檔全文（編譯時嵌入）；第一次啟動時原樣複製到 %APPDATA%。
pub const DEFAULT_SETTINGS_TEXT: &str = include_str!("../../config/default-settings.toml");

/// 設定檔檔名。
pub const SETTINGS_FILE_NAME: &str = "settings.toml";

/// 整份設定。數字一律用 i64（TOML 的整數型別），任何整數都讀得進來，再由 validate.rs 檢查範圍。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub schema_version: i64,
    pub general: GeneralSettings,
    pub display: DisplaySettings,
    pub colors: ColorSettings,
    pub borders: BorderSettings,
    pub providers: ProviderSettings,
}

/// [general] 一般設定。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GeneralSettings {
    pub language: String,
    pub autostart: bool,
    pub refresh_interval_minutes: i64,
    pub alert_threshold_percent: i64,
    pub query_timeout_seconds: i64,
    pub log_retention_days: i64,
}

/// [display] 顯示設定。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisplaySettings {
    pub mode: String,
    pub carousel_seconds: i64,
    pub carousel_animation: String,
    pub reset_time_format: String,
    pub theme: String,
    pub color_scheme: String,
    pub opacity_percent: i64,
}

/// [colors] 進度條漸變色：0% 的顏色、中間色、100% 的顏色，以及中間色的位置（mid_position，1～99 %）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ColorSettings {
    pub low: String,
    pub mid: String,
    pub high: String,
    pub mid_position: i64,
}

/// [borders] 警示外框與各主題卡片邊框（FR-18）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BorderSettings {
    pub alert_frame: bool,
    pub alert_color: String,
    pub card: CardBorderSettings,
}

/// [borders.card] 每個「主題＋色系」組合的卡片邊框：default／none／#RRGGBB。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CardBorderSettings {
    pub glass_dark: String,
    pub glass_light: String,
    pub minimal_dark: String,
    pub minimal_light: String,
    pub terminal_dark: String,
    pub terminal_light: String,
    pub gauge_dark: String,
    pub gauge_light: String,
    pub neon_dark: String,
}

/// [providers.*] 三家服務各自的設定。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderSettings {
    pub claude: SingleProviderSettings,
    pub codex: SingleProviderSettings,
    pub antigravity: SingleProviderSettings,
}

/// 單一服務的設定。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SingleProviderSettings {
    pub enabled: bool,
    pub cli_path: String,
    pub visible_quotas: Vec<String>,
}

/// 設定模組可能遇到的錯誤（只有「程式本身有問題」或「無法讀寫檔案」才會出現；
/// 使用者把設定檔改壞不算錯誤，而是用警告 + 預設值處理）。
#[derive(Debug)]
pub enum SettingsError {
    InvalidBundledDefaults(String),
    Io(std::io::Error),
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidBundledDefaults(detail) => {
                write!(formatter, "程式內建的預設設定格式不正確：{detail}")
            }
            Self::Io(error) => write!(formatter, "無法讀寫設定檔：{error}"),
        }
    }
}

impl std::error::Error for SettingsError {}

impl From<std::io::Error> for SettingsError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// 取得內建預設設定（解析 DEFAULT_SETTINGS_TEXT）。
pub fn default_settings() -> Result<Settings, SettingsError> {
    toml::from_str(DEFAULT_SETTINGS_TEXT)
        .map_err(|error| SettingsError::InvalidBundledDefaults(error.to_string()))
}

impl SingleProviderSettings {
    /// 依服務 ID（claude／codex／antigravity）取出該家的設定。
    pub fn for_id(
        providers: &ProviderSettings,
        id: crate::providers::ProviderId,
    ) -> &SingleProviderSettings {
        match id {
            crate::providers::ProviderId::Claude => &providers.claude,
            crate::providers::ProviderId::Codex => &providers.codex,
            crate::providers::ProviderId::Antigravity => &providers.antigravity,
        }
    }
}
