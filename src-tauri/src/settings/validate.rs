//! 檔案：settings/validate.rs
//! 用途：逐欄檢查設定值的範圍與格式（對應 41「讀取與驗證規則」與 FR-9）。
//!       不合法的欄位只有它自己改用預設值，並留下一筆警告；其他欄位照常使用。
//! 誰會用到：settings/load.rs（讀檔時）與 settings/store.rs（設定畫面按儲存時）。

use super::checks::{check_border, check_choice, check_cli_path, check_color, check_range};
use super::warning::{SettingsWarning, WarningKind};
use super::Settings;

/// 驗證結果：修正後的設定＋警告清單。
pub struct ValidatedSettings {
    pub settings: Settings,
    pub warnings: Vec<SettingsWarning>,
}

/// 檢查整份設定；每個不合法的欄位都改回 defaults 的值。
pub fn validate_settings(settings: Settings, defaults: &Settings) -> ValidatedSettings {
    let mut value = settings;
    let mut warnings = Vec::new();

    // ---------- 設定檔格式版本 ----------
    if value.schema_version != defaults.schema_version {
        warnings.push(SettingsWarning::new(
            "schema_version",
            WarningKind::UnsupportedSchema,
            &defaults.schema_version.to_string(),
            &defaults.schema_version.to_string(),
        ));
        value.schema_version = defaults.schema_version;
    }

    // ---------- [general] ----------
    check_choice(
        &mut value.general.language,
        &["auto", "zh-TW", "en"],
        &defaults.general.language,
        "general.language",
        &mut warnings,
    );
    check_range(
        &mut value.general.refresh_interval_minutes,
        1,
        120,
        defaults.general.refresh_interval_minutes,
        "general.refresh_interval_minutes",
        &mut warnings,
    );
    check_range(
        &mut value.general.alert_threshold_percent,
        50,
        99,
        defaults.general.alert_threshold_percent,
        "general.alert_threshold_percent",
        &mut warnings,
    );
    check_range(
        &mut value.general.query_timeout_seconds,
        10,
        120,
        defaults.general.query_timeout_seconds,
        "general.query_timeout_seconds",
        &mut warnings,
    );
    check_range(
        &mut value.general.log_retention_days,
        1,
        90,
        defaults.general.log_retention_days,
        "general.log_retention_days",
        &mut warnings,
    );

    // ---------- [display] ----------
    check_choice(
        &mut value.display.mode,
        &["carousel", "overview"],
        &defaults.display.mode,
        "display.mode",
        &mut warnings,
    );
    check_range(
        &mut value.display.carousel_seconds,
        3,
        60,
        defaults.display.carousel_seconds,
        "display.carousel_seconds",
        &mut warnings,
    );
    check_choice(
        &mut value.display.carousel_animation,
        &["slide", "flip"],
        &defaults.display.carousel_animation,
        "display.carousel_animation",
        &mut warnings,
    );
    check_choice(
        &mut value.display.reset_time_format,
        &["countdown", "absolute", "both"],
        &defaults.display.reset_time_format,
        "display.reset_time_format",
        &mut warnings,
    );
    check_choice(
        &mut value.display.theme,
        &["glass", "minimal", "terminal", "gauge", "neon"],
        &defaults.display.theme,
        "display.theme",
        &mut warnings,
    );
    check_choice(
        &mut value.display.color_scheme,
        &["auto", "light", "dark"],
        &defaults.display.color_scheme,
        "display.color_scheme",
        &mut warnings,
    );
    check_range(
        &mut value.display.opacity_percent,
        30,
        100,
        defaults.display.opacity_percent,
        "display.opacity_percent",
        &mut warnings,
    );

    // ---------- [colors] ----------
    check_color(
        &mut value.colors.low,
        &defaults.colors.low,
        "colors.low",
        &mut warnings,
    );
    check_color(
        &mut value.colors.mid,
        &defaults.colors.mid,
        "colors.mid",
        &mut warnings,
    );
    check_color(
        &mut value.colors.high,
        &defaults.colors.high,
        "colors.high",
        &mut warnings,
    );
    check_range(
        &mut value.colors.mid_position,
        1,
        99,
        defaults.colors.mid_position,
        "colors.mid_position",
        &mut warnings,
    );

    // ---------- [borders] ----------
    check_color(
        &mut value.borders.alert_color,
        &defaults.borders.alert_color,
        "borders.alert_color",
        &mut warnings,
    );
    let card = &mut value.borders.card;
    let default_card = &defaults.borders.card;
    let card_fields: [(&mut String, &String, &str); 9] = [
        (&mut card.glass_dark, &default_card.glass_dark, "borders.card.glass_dark"),
        (&mut card.glass_light, &default_card.glass_light, "borders.card.glass_light"),
        (&mut card.minimal_dark, &default_card.minimal_dark, "borders.card.minimal_dark"),
        (&mut card.minimal_light, &default_card.minimal_light, "borders.card.minimal_light"),
        (&mut card.terminal_dark, &default_card.terminal_dark, "borders.card.terminal_dark"),
        (&mut card.terminal_light, &default_card.terminal_light, "borders.card.terminal_light"),
        (&mut card.gauge_dark, &default_card.gauge_dark, "borders.card.gauge_dark"),
        (&mut card.gauge_light, &default_card.gauge_light, "borders.card.gauge_light"),
        (&mut card.neon_dark, &default_card.neon_dark, "borders.card.neon_dark"),
    ];
    for (field_value, default_value, field_name) in card_fields {
        check_border(field_value, default_value, field_name, &mut warnings);
    }

    // ---------- [providers.*] ----------
    check_cli_path(
        &mut value.providers.claude.cli_path,
        "providers.claude.cli_path",
        &mut warnings,
    );
    check_cli_path(
        &mut value.providers.codex.cli_path,
        "providers.codex.cli_path",
        &mut warnings,
    );
    check_cli_path(
        &mut value.providers.antigravity.cli_path,
        "providers.antigravity.cli_path",
        &mut warnings,
    );

    ValidatedSettings {
        settings: value,
        warnings,
    }
}
