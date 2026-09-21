//! 檔案：settings/writer.rs
//! 用途：把設定寫回 settings.toml，只改「值」，保留每一行的註解與排版（用 toml_edit）。
//!       舊版設定檔缺少的欄位或段落（例如後來才加入的 color_scheme、[borders]），從內建預設檔連同註解一起補上；
//!       已經移除的功能留下的舊段落（[background]，背景圖功能已移除）在儲存時刪掉，其他段落與註解不動。
//!       寫入前先把原檔備份成 settings.toml.bak，再用「暫存檔＋改名」安全寫入。
//! 誰會用到：settings/store.rs（設定畫面按儲存、⇄ 切換顯示方式）。

use super::{Settings, SettingsError, DEFAULT_SETTINGS_TEXT};
use crate::atomic_file;
use std::fs;
use std::path::{Path, PathBuf};
use toml_edit::{Array, DocumentMut, Item, Table, Value};

/// 已經移除的功能留下的舊段落名稱。讀檔時本來就會忽略（預設檔裡沒有的欄位一律忽略），
/// 按「儲存」寫回時從檔案刪掉，免得使用者以為改了還有作用。
/// - background：自訂背景圖（功能已移除，見 21 FR-16）
const REMOVED_SECTIONS: [&str; 1] = ["background"];

/// 完整寫回整份設定。
/// 原檔可以解析時，以原檔為樣板（保留使用者自己加的註解）；
/// 原檔壞掉或不存在時，以內建預設檔為樣板（使用者已在設定畫面按儲存，允許覆寫）。
pub fn write_settings(path: &Path, settings: &Settings) -> Result<(), SettingsError> {
    let mut document = read_document_or_default(path)?;
    let defaults = default_document()?;
    remove_obsolete_sections(document.as_table_mut());
    add_missing_from_defaults(document.as_table_mut(), defaults.as_table());
    fill_all_values(&mut document, settings);
    save_with_backup(path, &document)
}

/// 只改顯示方式（⇄ 按鈕）；原檔壞掉時不寫，交給呼叫端決定（回傳 Ok(false)）。
pub fn write_display_mode(path: &Path, mode: &str) -> Result<bool, SettingsError> {
    let original_text = fs::read_to_string(path)?;
    let Ok(mut document) = original_text.parse::<DocumentMut>() else {
        return Ok(false);
    };
    set_value(&mut document, &["display"], "mode", Value::from(mode));
    save_with_backup(path, &document)?;
    Ok(true)
}

/// 讀取原檔當樣板；不存在或格式錯誤就改用內建預設檔。
fn read_document_or_default(path: &Path) -> Result<DocumentMut, SettingsError> {
    if let Ok(text) = fs::read_to_string(path) {
        if let Ok(document) = text.parse::<DocumentMut>() {
            return Ok(document);
        }
    }
    default_document()
}

/// 內建預設設定檔（含註解）的 TOML 文件。
fn default_document() -> Result<DocumentMut, SettingsError> {
    DEFAULT_SETTINGS_TEXT
        .parse::<DocumentMut>()
        .map_err(|error| SettingsError::InvalidBundledDefaults(error.to_string()))
}

/// 刪掉已經移除的功能留下的舊段落（連同段落標題上方的註解）；其他段落與註解完全不動。
fn remove_obsolete_sections(target: &mut Table) {
    for name in REMOVED_SECTIONS {
        if target.remove(name).is_some() {
            log::info!("設定檔的 [{name}] 段落已不再使用，儲存時一併移除");
        }
    }
}

/// 使用者的設定檔缺少的欄位或段落，從預設檔連同註解一起補進來（已存在的欄位完全不動）。
/// 補進來的段落排在檔案最後面。
fn add_missing_from_defaults(target: &mut Table, defaults: &Table) {
    for (key, default_item) in defaults.iter() {
        if let Some(existing) = target.get_mut(key) {
            // 兩邊都有這個段落：往下一層檢查裡面缺了哪些欄位
            if let (Some(existing_table), Some(default_table)) =
                (existing.as_table_mut(), default_item.as_table())
            {
                add_missing_from_defaults(existing_table, default_table);
            }
            continue;
        }

        let mut new_item = default_item.clone();
        clear_table_positions(&mut new_item);
        // 連同欄位名稱前後的格式（上一行的說明註解）一起複製
        if let Some((default_key, _)) = defaults.get_key_value(key) {
            target.insert_formatted(default_key, new_item);
        }
    }
}

/// 清掉段落在預設檔裡的排列位置，讓補進來的段落排在使用者檔案的最後面，不會插進別的段落中間。
fn clear_table_positions(item: &mut Item) {
    if let Some(table) = item.as_table_mut() {
        table.set_position(None);
        for (_, child) in table.iter_mut() {
            clear_table_positions(child);
        }
    }
}

/// 把 Settings 的每個欄位填進 TOML 文件。
fn fill_all_values(document: &mut DocumentMut, settings: &Settings) {
    set_value(document, &[], "schema_version", Value::from(settings.schema_version));

    // [general]
    let general = &settings.general;
    set_value(document, &["general"], "language", Value::from(general.language.as_str()));
    set_value(document, &["general"], "autostart", Value::from(general.autostart));
    set_value(
        document,
        &["general"],
        "refresh_interval_minutes",
        Value::from(general.refresh_interval_minutes),
    );
    set_value(
        document,
        &["general"],
        "alert_threshold_percent",
        Value::from(general.alert_threshold_percent),
    );
    set_value(
        document,
        &["general"],
        "query_timeout_seconds",
        Value::from(general.query_timeout_seconds),
    );
    set_value(
        document,
        &["general"],
        "log_retention_days",
        Value::from(general.log_retention_days),
    );

    // [display]
    let display = &settings.display;
    set_value(document, &["display"], "mode", Value::from(display.mode.as_str()));
    set_value(
        document,
        &["display"],
        "carousel_seconds",
        Value::from(display.carousel_seconds),
    );
    set_value(
        document,
        &["display"],
        "carousel_animation",
        Value::from(display.carousel_animation.as_str()),
    );
    set_value(
        document,
        &["display"],
        "reset_time_format",
        Value::from(display.reset_time_format.as_str()),
    );
    set_value(document, &["display"], "theme", Value::from(display.theme.as_str()));
    set_value(
        document,
        &["display"],
        "color_scheme",
        Value::from(display.color_scheme.as_str()),
    );
    set_value(
        document,
        &["display"],
        "opacity_percent",
        Value::from(display.opacity_percent),
    );

    // [colors]
    let colors = &settings.colors;
    set_value(document, &["colors"], "low", Value::from(colors.low.as_str()));
    set_value(document, &["colors"], "mid", Value::from(colors.mid.as_str()));
    set_value(document, &["colors"], "high", Value::from(colors.high.as_str()));
    set_value(document, &["colors"], "mid_position", Value::from(colors.mid_position));

    // [borders] 與 [borders.card]
    let borders = &settings.borders;
    set_value(document, &["borders"], "alert_frame", Value::from(borders.alert_frame));
    set_value(
        document,
        &["borders"],
        "alert_color",
        Value::from(borders.alert_color.as_str()),
    );
    let card = &borders.card;
    let card_values: [(&str, &String); 9] = [
        ("glass_dark", &card.glass_dark),
        ("glass_light", &card.glass_light),
        ("minimal_dark", &card.minimal_dark),
        ("minimal_light", &card.minimal_light),
        ("terminal_dark", &card.terminal_dark),
        ("terminal_light", &card.terminal_light),
        ("gauge_dark", &card.gauge_dark),
        ("gauge_light", &card.gauge_light),
        ("neon_dark", &card.neon_dark),
    ];
    for (key, value) in card_values {
        set_value(document, &["borders", "card"], key, Value::from(value.as_str()));
    }

    // [providers.*]
    let providers = [
        ("claude", &settings.providers.claude),
        ("codex", &settings.providers.codex),
        ("antigravity", &settings.providers.antigravity),
    ];
    for (name, provider) in providers {
        let section = ["providers", name];
        set_value(document, &section, "enabled", Value::from(provider.enabled));
        set_value(document, &section, "cli_path", Value::from(provider.cli_path.as_str()));

        let mut quota_array = Array::new();
        for quota_id in &provider.visible_quotas {
            quota_array.push(quota_id.as_str());
        }
        set_value(document, &section, "visible_quotas", Value::Array(quota_array));
    }
}

/// 設定一個值；原本已有這個欄位時，沿用它前後的空白與行尾註解，只換掉值本身。
fn set_value(document: &mut DocumentMut, section_path: &[&str], key: &str, new_value: Value) {
    let table = section_table(document, section_path);
    let mut value_to_write = new_value;

    if let Some(existing) = table.get(key).and_then(Item::as_value) {
        // 行尾註解（例如 `# 自動更新間隔…`）存在值的 decor 裡，複製過去才不會消失
        let old_decor = existing.decor().clone();
        *value_to_write.decor_mut() = old_decor;
    }

    table.insert(key, Item::Value(value_to_write));
}

/// 找到（或建立）section_path 指到的表格，例如 ["providers", "claude"]。
fn section_table<'a>(document: &'a mut DocumentMut, section_path: &[&str]) -> &'a mut Table {
    let mut table = document.as_table_mut();
    for name in section_path {
        let needs_new_table = !matches!(table.get(name), Some(Item::Table(_)));
        if needs_new_table {
            table.insert(name, Item::Table(Table::new()));
        }
        table = table
            .get_mut(name)
            .and_then(Item::as_table_mut)
            .expect("上一行已確保這一層是表格");
    }
    table
}

/// 先備份原檔成 settings.toml.bak，再安全寫入新內容。
fn save_with_backup(path: &Path, document: &DocumentMut) -> Result<(), SettingsError> {
    if path.exists() {
        fs::copy(path, backup_path(path))?;
    }
    atomic_file::write_atomic(path, document.to_string().as_bytes())?;
    Ok(())
}

/// settings.toml → settings.toml.bak
pub fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".bak");
    PathBuf::from(name)
}
