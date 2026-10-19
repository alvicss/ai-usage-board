//! 檔案：settings/tests.rs
//! 用途：settings 模組的單元測試：第一次建立、壞檔、缺欄位、型別錯、超範圍、保留註解寫回、舊版檔案補欄位、
//!       已移除的舊段落（[background]）讀取時忽略、儲存時刪掉。
//!       公開版沒有 docs/，所以拿掉「預設設定檔與 docs/41_DATA_MODEL.md 一致」的測試（該測試只留在私有開發版）。
//! 誰會用到：`cargo test`；只用暫存資料夾，不碰使用者真正的 %APPDATA%。

use super::load::{load_from_text, load_or_create};
use super::warning::WarningKind;
use super::writer::{backup_path, write_display_mode, write_settings};
use super::{default_settings, SettingsStore, DEFAULT_SETTINGS_TEXT};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// 建立一個每次都不同名稱的暫存資料夾，測試結束自行刪除。
fn temporary_directory(test_name: &str) -> PathBuf {
    let unique_number = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("系統時間應晚於 UNIX epoch")
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("ai-usage-board-{test_name}-{unique_number}"));
    fs::create_dir_all(&directory).expect("應建立測試資料夾");
    directory
}

#[test]
fn bundled_defaults_match_the_data_model() {
    let defaults = default_settings().expect("內建預設設定應可解析");
    assert_eq!(defaults.schema_version, 1);
    assert_eq!(defaults.general.refresh_interval_minutes, 10);
    assert_eq!(defaults.general.alert_threshold_percent, 80);
    assert_eq!(defaults.general.query_timeout_seconds, 45);
    assert_eq!(defaults.display.theme, "glass");
    assert_eq!(defaults.display.color_scheme, "auto");
    assert_eq!(defaults.colors.mid_position, 60);
    assert!(defaults.borders.alert_frame);
    assert_eq!(defaults.borders.alert_color, "#FF3B3B");
    assert_eq!(defaults.borders.card.neon_dark, "default");
    assert_eq!(
        defaults.providers.antigravity.visible_quotas.len(),
        5,
        "Antigravity 預設顯示五個額度（Gemini、Claude+GPT 各兩個，加上 Argon 每週）"
    );
}

#[test]
fn first_load_creates_the_exact_bundled_settings_file() {
    let directory = temporary_directory("first-load");
    let path = directory.join("settings.toml");
    let loaded = load_or_create(&path).expect("第一次載入應成功");
    let file_text = fs::read_to_string(&path).expect("應建立 settings.toml");

    assert_eq!(file_text, DEFAULT_SETTINGS_TEXT);
    assert!(loaded.warnings.is_empty());
    assert!(!loaded.file_is_broken);

    fs::remove_dir_all(directory).expect("應清除測試資料夾");
}

#[test]
fn out_of_range_value_uses_only_that_fields_default() {
    let changed_text = DEFAULT_SETTINGS_TEXT
        .replace("refresh_interval_minutes = 10", "refresh_interval_minutes = 0")
        .replace("carousel_seconds = 8", "carousel_seconds = 15");

    let loaded = load_from_text(&changed_text).expect("載入應成功");

    assert_eq!(loaded.settings.general.refresh_interval_minutes, 10);
    assert_eq!(loaded.settings.display.carousel_seconds, 15);
    assert_eq!(loaded.warnings.len(), 1);
    assert_eq!(loaded.warnings[0].kind, WarningKind::OutOfRange);
    assert_eq!(loaded.warnings[0].field, "general.refresh_interval_minutes");
    // 範圍用結構化欄位給畫面，畫面再依語言組成「1～120」或「1 to 120」（FR-17）
    assert_eq!(loaded.warnings[0].minimum, Some(1));
    assert_eq!(loaded.warnings[0].maximum, Some(120));
}

#[test]
fn warnings_carry_choices_and_types_for_the_screen() {
    let changed_text = DEFAULT_SETTINGS_TEXT
        .replace("color_scheme = \"auto\"", "color_scheme = \"purple\"")
        .replace("autostart = false", "autostart = \"yes\"");

    let loaded = load_from_text(&changed_text).expect("載入應成功");

    let mut found_choice = false;
    let mut found_type = false;
    for warning in &loaded.warnings {
        if warning.field == "display.color_scheme" {
            assert_eq!(warning.kind, WarningKind::InvalidChoice);
            assert_eq!(warning.choices, vec!["auto", "light", "dark"]);
            found_choice = true;
        }
        if warning.field == "general.autostart" {
            assert_eq!(warning.kind, WarningKind::WrongType);
            assert_eq!(warning.value_type.as_deref(), Some("boolean"));
            found_type = true;
        }
    }
    assert!(found_choice && found_type);
}

/// 舊版（有背景圖功能時）的設定檔：[background] 段落接在 [display] 後面，還有使用者自己寫的註解。
fn settings_text_with_old_background_section() -> String {
    let background_block = "
# 我自己的背景圖筆記
[background]
image = \"background-1.png\"   # 背景圖檔名
dim_percent = 0
blur_px = 20
";
    DEFAULT_SETTINGS_TEXT.replace("
[colors]", &format!("{background_block}
# 我自己的配色筆記
[colors]"))
}

#[test]
fn old_background_section_is_ignored_when_loading() {
    let loaded = load_from_text(&settings_text_with_old_background_section()).expect("載入應成功");

    assert!(!loaded.file_is_broken);
    assert!(loaded.warnings.is_empty(), "已移除的 [background] 不算錯誤：{:?}", loaded.warnings);
    assert_eq!(loaded.settings, default_settings().expect("預設設定應有效"));
}

#[test]
fn saving_removes_old_background_section_and_keeps_other_comments() {
    let directory = temporary_directory("remove-background");
    let path = directory.join("settings.toml");
    fs::write(&path, settings_text_with_old_background_section()).expect("應寫入舊版設定");

    let mut settings = default_settings().expect("預設設定應有效");
    settings.display.opacity_percent = 80;
    write_settings(&path, &settings).expect("寫回應成功");

    let written = fs::read_to_string(&path).expect("應可讀回");
    assert!(!written.contains("[background]"), "[background] 段落要移除");
    assert!(!written.contains("dim_percent"));
    assert!(!written.contains("blur_px"));
    assert!(written.contains("# 我自己的配色筆記"), "其他段落的註解要保留");
    assert!(written.contains("opacity_percent = 80"));
    assert!(written.contains("# 整體不透明度 30～100"), "欄位的行尾註解要保留");

    let reloaded = load_from_text(&written).expect("寫回的檔案應可再讀");
    assert!(reloaded.warnings.is_empty());
    assert_eq!(reloaded.settings, settings);

    fs::remove_dir_all(directory).expect("應清除測試資料夾");
}

#[test]
fn wrong_type_uses_only_that_fields_default() {
    // 數字欄位填了文字、u8 範圍外的大數字：都不能讓整份設定失效
    let changed_text = DEFAULT_SETTINGS_TEXT
        .replace("alert_threshold_percent = 80", "alert_threshold_percent = \"很多\"")
        .replace("opacity_percent = 100", "opacity_percent = 99999")
        .replace("carousel_seconds = 8", "carousel_seconds = 20");

    let loaded = load_from_text(&changed_text).expect("載入應成功");

    assert!(!loaded.file_is_broken);
    assert_eq!(loaded.settings.general.alert_threshold_percent, 80);
    assert_eq!(loaded.settings.display.opacity_percent, 100);
    assert_eq!(loaded.settings.display.carousel_seconds, 20);
    let kinds: Vec<WarningKind> = loaded.warnings.iter().map(|warning| warning.kind).collect();
    assert!(kinds.contains(&WarningKind::WrongType));
    assert!(kinds.contains(&WarningKind::OutOfRange));
}

#[test]
fn missing_fields_use_defaults_without_warnings() {
    // 舊版設定檔沒有 color_scheme 與 [borders]，應直接補預設值、不算錯誤
    let old_text = "schema_version = 1\n[display]\ntheme = \"minimal\"\n";

    let loaded = load_from_text(old_text).expect("載入應成功");

    assert!(loaded.warnings.is_empty());
    assert_eq!(loaded.settings.display.theme, "minimal");
    assert_eq!(loaded.settings.display.color_scheme, "auto");
    assert_eq!(loaded.settings.borders.alert_color, "#FF3B3B");
}

#[test]
fn invalid_choices_colors_and_borders_fall_back() {
    let changed_text = DEFAULT_SETTINGS_TEXT
        .replace("color_scheme = \"auto\"", "color_scheme = \"purple\"")
        .replace("low = \"#01B468\"", "low = \"green\"")
        .replace("minimal_dark = \"default\"", "minimal_dark = \"#12345\"")
        .replace("minimal_light = \"default\"", "minimal_light = \"#A1B2C3\"");

    let loaded = load_from_text(&changed_text).expect("載入應成功");

    assert_eq!(loaded.settings.display.color_scheme, "auto");
    assert_eq!(loaded.settings.colors.low, "#01B468");
    assert_eq!(loaded.settings.borders.card.minimal_dark, "default");
    assert_eq!(loaded.settings.borders.card.minimal_light, "#A1B2C3");
    assert_eq!(loaded.warnings.len(), 3);
}

#[test]
fn missing_cli_path_falls_back_to_auto_detect() {
    let changed_text = DEFAULT_SETTINGS_TEXT.replacen(
        "cli_path = \"\"",
        "cli_path = \"Z:\\\\missing\\\\claude.exe\"",
        1,
    );

    let loaded = load_from_text(&changed_text).expect("載入應成功");

    assert_eq!(loaded.settings.providers.claude.cli_path, "");
    assert_eq!(loaded.warnings.len(), 1);
    assert_eq!(loaded.warnings[0].kind, WarningKind::InvalidCliPath);
}

#[test]
fn broken_toml_is_preserved_and_defaults_are_used() {
    let directory = temporary_directory("broken-file");
    let path = directory.join("settings.toml");
    let broken_text = "schema_version = 1\n[general\nrefresh_interval_minutes = 10\n";
    fs::write(&path, broken_text).expect("應寫入壞掉的測試設定");

    let loaded = load_or_create(&path).expect("壞檔時仍應安全啟動");
    let preserved_text = fs::read_to_string(&path).expect("原始壞檔應保留");

    assert_eq!(preserved_text, broken_text);
    assert!(loaded.file_is_broken);
    assert_eq!(loaded.settings.general.refresh_interval_minutes, 10);
    assert_eq!(loaded.warnings.len(), 1);
    assert_eq!(loaded.warnings[0].kind, WarningKind::InvalidFile);
    assert_eq!(loaded.warnings[0].line, Some(2));

    fs::remove_dir_all(directory).expect("應清除測試資料夾");
}

#[test]
fn broken_value_line_reports_the_field_name() {
    let broken_text = "[general]\nlanguage = \"auto\"\nrefresh_interval_minutes = 1 0\n";

    let loaded = load_from_text(broken_text).expect("壞檔時仍應回傳預設值");

    assert_eq!(loaded.warnings[0].line, Some(3));
    assert_eq!(loaded.warnings[0].field, "refresh_interval_minutes");
}

#[test]
fn writing_keeps_comments_and_creates_backup() {
    let directory = temporary_directory("write-keeps-comments");
    let path = directory.join("settings.toml");
    fs::write(&path, DEFAULT_SETTINGS_TEXT).expect("應寫入預設設定");

    let mut settings = default_settings().expect("預設設定應有效");
    settings.general.refresh_interval_minutes = 3;
    settings.display.theme = "minimal".to_string();
    settings.borders.card.minimal_dark = "none".to_string();
    settings.providers.codex.visible_quotas = vec!["codex.primary".to_string()];
    write_settings(&path, &settings).expect("寫回應成功");

    let written = fs::read_to_string(&path).expect("應可讀回");
    assert!(written.contains("refresh_interval_minutes = 3"));
    assert!(written.contains("# 自動更新間隔（分鐘）1～120"));
    assert!(written.contains("theme = \"minimal\""));
    assert!(written.contains("# 各主題卡片邊框"));
    assert!(written.contains("minimal_dark = \"none\""));
    assert!(written.contains("visible_quotas = [\"codex.primary\"]"));
    assert!(backup_path(&path).exists(), "寫回前應先備份 .bak");

    let reloaded = load_from_text(&written).expect("寫回的檔案應可再讀");
    assert!(reloaded.warnings.is_empty());
    assert_eq!(reloaded.settings, settings);

    fs::remove_dir_all(directory).expect("應清除測試資料夾");
}

#[test]
fn saving_an_old_file_adds_missing_fields_with_their_comments() {
    // 舊版設定檔：沒有 color_scheme，也沒有 [borders] 段落
    let directory = temporary_directory("write-old-file");
    let path = directory.join("settings.toml");
    let mut old_text = String::new();
    for line in DEFAULT_SETTINGS_TEXT.lines() {
        if line.starts_with("color_scheme") {
            continue;
        }
        if line.starts_with("[borders") {
            break;
        }
        old_text.push_str(line);
        old_text.push('\n');
    }
    old_text.push_str("\n[providers.claude]\nenabled = true\ncli_path = \"\"\n");
    old_text.push_str("visible_quotas = [\"claude.five_hour\"]   # 自己寫的註解\n");
    fs::write(&path, &old_text).expect("應寫入舊版設定");

    let mut settings = default_settings().expect("預設設定應有效");
    settings.borders.card.glass_dark = "none".to_string();
    write_settings(&path, &settings).expect("寫回應成功");

    let written = fs::read_to_string(&path).expect("應可讀回");
    assert!(written.contains("# 自己寫的註解"), "使用者自己的註解要保留");
    assert!(written.contains("# 色系 Color scheme"), "補上的欄位要帶預設註解");
    assert!(written.contains("# 各主題卡片邊框"), "補上的段落要帶預設註解");
    assert!(written.contains("glass_dark = \"none\""));
    let borders_at = written.find("[borders]").expect("應補上 [borders]");
    let claude_at = written.find("[providers.claude]").expect("原本的段落還在");
    assert!(borders_at > claude_at, "補上的段落排在最後面");

    let reloaded = load_from_text(&written).expect("寫回的檔案應可再讀");
    assert!(reloaded.warnings.is_empty());
    assert_eq!(reloaded.settings, settings);

    fs::remove_dir_all(directory).expect("應清除測試資料夾");
}

#[test]
fn display_mode_is_written_but_broken_file_is_not_touched() {
    let directory = temporary_directory("display-mode");
    let path = directory.join("settings.toml");

    fs::write(&path, DEFAULT_SETTINGS_TEXT).expect("應寫入預設設定");
    let written = write_display_mode(&path, "overview").expect("寫入應成功");
    assert!(written);
    let text = fs::read_to_string(&path).expect("應可讀回");
    assert!(text.contains("mode = \"overview\""));
    assert!(text.contains("# 顯示方式：carousel（輪播）/ overview（並排）"));

    let broken_text = "[display\nmode = \"carousel\"\n";
    fs::write(&path, broken_text).expect("應寫入壞檔");
    let store = SettingsStore::load(&path).expect("壞檔也應能載入");
    let settings = store
        .set_display_mode("overview")
        .expect("切換顯示方式應成功");
    assert_eq!(settings.display.mode, "overview");
    let untouched = fs::read_to_string(&path).expect("應可讀回");
    assert_eq!(untouched, broken_text, "壞檔在按儲存前不得被覆寫");

    fs::remove_dir_all(directory).expect("應清除測試資料夾");
}

#[test]
fn saving_rejects_invalid_fields_and_repairs_broken_file() {
    let directory = temporary_directory("save");
    let path = directory.join("settings.toml");
    fs::write(&path, "[general\n").expect("應寫入壞檔");
    let store = SettingsStore::load(&path).expect("壞檔也應能載入");

    let mut invalid = store.current();
    invalid.general.refresh_interval_minutes = 500;
    let error = store.save(invalid).expect_err("超範圍的值不能儲存");
    assert_eq!(error.warnings.len(), 1);

    let mut valid = store.current();
    valid.general.refresh_interval_minutes = 5;
    store.save(valid).expect("合法設定應能儲存");
    let reloaded = load_or_create(&path).expect("儲存後應可讀回");
    assert!(!reloaded.file_is_broken);
    assert_eq!(reloaded.settings.general.refresh_interval_minutes, 5);
    assert!(store.response().warnings.is_empty());

    fs::remove_dir_all(directory).expect("應清除測試資料夾");
}
