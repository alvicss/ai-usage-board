//! 檔案：settings/load.rs
//! 用途：讀取 settings.toml（沒有就從內建預設檔建立），和預設值合併，並做「型別」檢查。
//!       規則（41「讀取與驗證規則」）：
//!       - 整個檔案格式錯誤 → 全部用預設值，警告指出行號；原檔不動。
//!       - 缺少某個欄位 → 該欄位用預設值，不算錯誤。
//!       - 某欄位型別不對 → 只有那個欄位用預設值，並產生警告。
//! 誰會用到：settings/store.rs（程式啟動時）與單元測試。

use super::validate::validate_settings;
use super::warning::{SettingsWarning, WarningKind};
use super::{default_settings, Settings, SettingsError, DEFAULT_SETTINGS_TEXT};
use std::fs;
use std::path::Path;

/// 讀檔結果：最後採用的設定、所有警告，以及「整個檔案是否壞掉」。
#[derive(Debug, Clone)]
pub struct LoadedSettings {
    pub settings: Settings,
    pub warnings: Vec<SettingsWarning>,
    /// true 代表 settings.toml 整份無法解析；此時使用者按「儲存」之前都不能覆寫它
    pub file_is_broken: bool,
}

/// 讀取設定檔；第一次啟動時先把內建預設檔原樣複製過去。
/// 輸入：settings.toml 的完整路徑。
pub fn load_or_create(settings_path: &Path) -> Result<LoadedSettings, SettingsError> {
    if let Some(directory) = settings_path.parent() {
        fs::create_dir_all(directory)?;
    }
    if !settings_path.exists() {
        fs::write(settings_path, DEFAULT_SETTINGS_TEXT.as_bytes())?;
    }

    let user_text = fs::read_to_string(settings_path)?;
    load_from_text(&user_text)
}

/// 從設定檔文字產生最後採用的設定（和檔案位置無關，方便單元測試）。
pub fn load_from_text(user_text: &str) -> Result<LoadedSettings, SettingsError> {
    let defaults = default_settings()?;
    let default_table = toml::from_str::<toml::Table>(DEFAULT_SETTINGS_TEXT)
        .map_err(|error| SettingsError::InvalidBundledDefaults(error.to_string()))?;

    // 第 1 步：整份檔案能不能解析成 TOML
    let user_table = match toml::from_str::<toml::Table>(user_text) {
        Ok(table) => table,
        Err(error) => {
            let warning = invalid_file_warning(user_text, &error);
            return Ok(LoadedSettings {
                settings: defaults,
                warnings: vec![warning],
                file_is_broken: true,
            });
        }
    };

    // 第 2 步：以預設值為底，逐欄覆蓋使用者的值；型別不對的欄位保留預設值
    let mut warnings = Vec::new();
    let mut merged = toml::Value::Table(default_table);
    merge_user_values(&mut merged, toml::Value::Table(user_table), "", &mut warnings);

    // 第 3 步：轉成 Settings（型別已檢查過，照理不會失敗；萬一失敗就整份用預設值）
    let merged_settings = match merged.try_into::<Settings>() {
        Ok(settings) => settings,
        Err(error) => {
            log::warn!("設定合併後仍無法轉換：{error}");
            let mut warning =
                SettingsWarning::new("settings.toml", WarningKind::InvalidFile, "", "");
            warning.line = Some(1);
            return Ok(LoadedSettings {
                settings: defaults,
                warnings: vec![warning],
                file_is_broken: true,
            });
        }
    };

    // 第 4 步：檢查每個欄位的範圍與格式
    let validated = validate_settings(merged_settings, &defaults);
    warnings.extend(validated.warnings);

    Ok(LoadedSettings {
        settings: validated.settings,
        warnings,
        file_is_broken: false,
    })
}

/// 把使用者的值逐一蓋到預設值上。
/// - 預設值裡沒有的欄位：忽略（可能是打錯字或舊版欄位）。
/// - 型別和預設值不同：保留預設值並記一筆警告。
fn merge_user_values(
    base: &mut toml::Value,
    user: toml::Value,
    path: &str,
    warnings: &mut Vec<SettingsWarning>,
) {
    // 兩邊都是表格（例如 [general]）：往下一層逐欄處理
    if let (toml::Value::Table(base_table), toml::Value::Table(user_table)) = (&mut *base, &user)
    {
        for (key, user_value) in user_table {
            let child_path = if path.is_empty() {
                key.clone()
            } else {
                format!("{path}.{key}")
            };
            if let Some(base_value) = base_table.get_mut(key) {
                merge_user_values(base_value, user_value.clone(), &child_path, warnings);
            }
        }
        return;
    }

    // 單一值：型別相同才採用
    if same_kind(base, &user) {
        *base = user;
    } else {
        let warning = SettingsWarning::new(
            path,
            WarningKind::WrongType,
            kind_name(base),
            &display_value(base),
        );
        warnings.push(warning.with_value_type(kind_code(base)));
    }
}

/// 判斷兩個值是不是同一種型別；陣列還要求每個元素都是文字（本案唯一的陣列是 visible_quotas）。
fn same_kind(expected: &toml::Value, actual: &toml::Value) -> bool {
    match (expected, actual) {
        (toml::Value::String(_), toml::Value::String(_)) => true,
        (toml::Value::Integer(_), toml::Value::Integer(_)) => true,
        (toml::Value::Boolean(_), toml::Value::Boolean(_)) => true,
        (toml::Value::Array(_), toml::Value::Array(items)) => {
            let mut all_strings = true;
            for item in items {
                if !item.is_str() {
                    all_strings = false;
                }
            }
            all_strings
        }
        _ => false,
    }
}

/// 型別的代號（給警告的 value_type 欄位用；畫面依語言翻譯成「整數」或 "whole number"）。
fn kind_code(value: &toml::Value) -> &'static str {
    match value {
        toml::Value::String(_) => "string",
        toml::Value::Integer(_) => "integer",
        toml::Value::Boolean(_) => "boolean",
        toml::Value::Array(_) => "string_list",
        toml::Value::Table(_) => "table",
        _ => "other",
    }
}

/// 型別的中文名稱（給警告的 expected 欄位，也就是 log 用）。
fn kind_name(value: &toml::Value) -> &'static str {
    match value {
        toml::Value::String(_) => "文字",
        toml::Value::Integer(_) => "整數",
        toml::Value::Boolean(_) => "true／false",
        toml::Value::Array(_) => "文字清單",
        toml::Value::Table(_) => "區段",
        _ => "其他",
    }
}

/// 把預設值轉成顯示用文字（字串不加引號，方便閱讀）。
fn display_value(value: &toml::Value) -> String {
    match value {
        toml::Value::String(text) => text.clone(),
        toml::Value::Table(_) => "整個區段的預設值".to_string(),
        other => other.to_string(),
    }
}

/// 整份檔案格式錯誤時的警告：找出行號，行內有「欄位 = 值」就一併指出欄位名稱。
fn invalid_file_warning(source: &str, error: &toml::de::Error) -> SettingsWarning {
    let mut line_number = 1;
    if let Some(span) = error.span() {
        // 錯誤位置之前有幾個換行，就代表在第幾行
        let start = span.start.min(source.len());
        let mut count = 1;
        for character in source[..start].chars() {
            if character == '\n' {
                count += 1;
            }
        }
        line_number = count;
    }

    let mut field = "settings.toml".to_string();
    if let Some(line_text) = source.lines().nth(line_number - 1) {
        if let Some((key, _)) = line_text.split_once('=') {
            let key = key.trim();
            if !key.is_empty() && !key.starts_with('#') {
                field = key.to_string();
            }
        }
    }

    let mut warning = SettingsWarning::new(&field, WarningKind::InvalidFile, "", "");
    warning.line = Some(line_number);
    warning
}
