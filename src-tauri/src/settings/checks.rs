//! 檔案：settings/checks.rs
//! 用途：單一欄位的檢查工具（範圍、選項、顏色、邊框、CLI 路徑）。
//!       每個函式遇到不合法的值，就把它改回預設值並加一筆警告；合法就什麼都不做。
//! 誰會用到：settings/validate.rs。

use super::warning::{SettingsWarning, WarningKind};
use std::path::Path;

/// 數字必須在 minimum～maximum 之間（含兩端），否則改回預設值。
pub fn check_range(
    value: &mut i64,
    minimum: i64,
    maximum: i64,
    default_value: i64,
    field: &str,
    warnings: &mut Vec<SettingsWarning>,
) {
    if *value >= minimum && *value <= maximum {
        return;
    }
    let warning = SettingsWarning::new(
        field,
        WarningKind::OutOfRange,
        &format!("{minimum}～{maximum}"),
        &default_value.to_string(),
    );
    warnings.push(warning.with_range(minimum, maximum));
    *value = default_value;
}

/// 文字必須是允許的選項之一，否則改回預設值。
pub fn check_choice(
    value: &mut String,
    allowed_values: &[&str],
    default_value: &str,
    field: &str,
    warnings: &mut Vec<SettingsWarning>,
) {
    if allowed_values.contains(&value.as_str()) {
        return;
    }
    let warning = SettingsWarning::new(
        field,
        WarningKind::InvalidChoice,
        &allowed_values.join("、"),
        default_value,
    );
    warnings.push(warning.with_choices(allowed_values));
    *value = default_value.to_string();
}

/// 判斷文字是不是 #RRGGBB 格式的顏色。
pub fn is_hex_color(value: &str) -> bool {
    if value.len() != 7 || !value.starts_with('#') {
        return false;
    }
    let mut all_hex = true;
    for character in value[1..].chars() {
        if !character.is_ascii_hexdigit() {
            all_hex = false;
        }
    }
    all_hex
}

/// 顏色必須是 #RRGGBB，否則改回預設值。
pub fn check_color(
    value: &mut String,
    default_value: &str,
    field: &str,
    warnings: &mut Vec<SettingsWarning>,
) {
    if is_hex_color(value) {
        return;
    }
    warnings.push(SettingsWarning::new(
        field,
        WarningKind::InvalidColor,
        "#RRGGBB",
        default_value,
    ));
    *value = default_value.to_string();
}

/// 卡片邊框只能是 default、none 或 #RRGGBB，否則改回預設值。
pub fn check_border(
    value: &mut String,
    default_value: &str,
    field: &str,
    warnings: &mut Vec<SettingsWarning>,
) {
    if value == "default" || value == "none" || is_hex_color(value) {
        return;
    }
    warnings.push(SettingsWarning::new(
        field,
        WarningKind::InvalidBorder,
        "default、none、#RRGGBB",
        default_value,
    ));
    *value = default_value.to_string();
}

/// CLI 路徑：空白（自動偵測），或存在的 .exe／.cmd 檔案；否則清空改回自動偵測。
pub fn check_cli_path(value: &mut String, field: &str, warnings: &mut Vec<SettingsWarning>) {
    if value.is_empty() {
        return;
    }
    let path = Path::new(value.as_str());
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    let has_allowed_extension =
        extension.eq_ignore_ascii_case("exe") || extension.eq_ignore_ascii_case("cmd");
    if has_allowed_extension && path.is_file() {
        return;
    }
    warnings.push(SettingsWarning::new(
        field,
        WarningKind::InvalidCliPath,
        ".exe／.cmd",
        "",
    ));
    value.clear();
}
