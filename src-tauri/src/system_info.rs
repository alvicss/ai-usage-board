//! 檔案：system_info.rs
//! 用途：提供畫面需要知道的「這台電腦的環境」（指令 get_system_info）：
//!       1. 語言設定為「自動」時要用哪種語言（FR-17）：
//!          Windows 顯示語言是中文，或 Windows 地區是台灣、香港、澳門 → 繁體中文；其他 → 英文。
//!       2. 這台 Windows 能不能用玻璃主題的 Acrylic 材質（不能的話，畫面把玻璃卡片改成半透明底，43「玻璃材質」）。
//!       讀 Windows 設定的部分在 desktop.rs；這裡只做判斷（純函式，有單元測試）。
//!       另外提供 resolve_language：把設定的 auto／zh-TW／en 換成實際語言（Rust 端的系統匣選單用）。
//! 誰會用到：lib.rs 註冊指令；畫面的 services/backend.ts（getSystemInfo）；tray.rs（系統匣選單的語言）。

use crate::desktop;
use serde::Serialize;

/// 「語言：自動」時會用繁體中文的地區（Windows 設定的國家或地區，兩碼代號）：台灣、香港、澳門。
const TRADITIONAL_CHINESE_REGIONS: [&str; 3] = ["TW", "HK", "MO"];

/// get_system_info 的回傳內容。
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SystemInfo {
    /// 語言設定為「自動」時要用的語言："zh-TW" 或 "en"
    pub language: String,
    /// 玻璃主題能不能套用 Acrylic 材質（Windows 11 22H2 以上才有）
    pub acrylic: bool,
}

/// 依 Windows 的顯示語言與地區，決定「自動」時的介面語言。
/// 輸入：顯示語言是不是中文、地區兩碼代號（例如 "TW"；讀不到時是空字串）。
/// 輸出："zh-TW" 或 "en"。
pub fn choose_language(ui_language_is_chinese: bool, region_code: &str) -> &'static str {
    // 顯示語言是中文（繁體、簡體都算）：用繁體中文
    if ui_language_is_chinese {
        return "zh-TW";
    }
    // 顯示語言不是中文，但地區設在台灣、香港、澳門：也用繁體中文
    let region = region_code.trim().to_ascii_uppercase();
    for chinese_region in TRADITIONAL_CHINESE_REGIONS {
        if region == chinese_region {
            return "zh-TW";
        }
    }
    "en"
}

/// 設定的語言換成實際要用的語言：手動選的 zh-TW／en 直接用；auto（或看不懂的值）依 Windows 判斷。
/// 輸入：設定的語言、「自動」時的判斷結果。輸出："zh-TW" 或 "en"（純函式，方便測試）。
pub fn resolve_language_with(language_setting: &str, automatic_language: &'static str) -> &'static str {
    match language_setting {
        "zh-TW" => "zh-TW",
        "en" => "en",
        _ => automatic_language,
    }
}

/// 設定的語言換成實際要用的語言（auto 時讀 Windows 的顯示語言與地區）。
pub fn resolve_language(language_setting: &str) -> &'static str {
    let automatic_language =
        choose_language(desktop::ui_language_is_chinese(), &desktop::home_region_code());
    resolve_language_with(language_setting, automatic_language)
}

/// 畫面啟動時取得這台電腦的環境（語言「自動」的結果、能不能用 Acrylic）。
#[tauri::command]
pub fn get_system_info() -> SystemInfo {
    let language = choose_language(desktop::ui_language_is_chinese(), &desktop::home_region_code());
    SystemInfo {
        language: language.to_string(),
        acrylic: desktop::acrylic_supported(),
    }
}

#[cfg(test)]
mod tests {
    use super::{choose_language, resolve_language_with};

    #[test]
    fn manual_language_wins_over_automatic() {
        assert_eq!(resolve_language_with("zh-TW", "en"), "zh-TW");
        assert_eq!(resolve_language_with("en", "zh-TW"), "en");
        assert_eq!(resolve_language_with("auto", "zh-TW"), "zh-TW");
        assert_eq!(resolve_language_with("auto", "en"), "en");
    }

    #[test]
    fn chinese_display_language_uses_traditional_chinese() {
        assert_eq!(choose_language(true, "US"), "zh-TW");
        assert_eq!(choose_language(true, "CN"), "zh-TW");
        assert_eq!(choose_language(true, ""), "zh-TW");
    }

    #[test]
    fn taiwan_hong_kong_and_macao_regions_use_traditional_chinese() {
        // 顯示語言英文、地區台灣時，應判定為繁體中文
        assert_eq!(choose_language(false, "TW"), "zh-TW");
        assert_eq!(choose_language(false, "HK"), "zh-TW");
        assert_eq!(choose_language(false, "MO"), "zh-TW");
        assert_eq!(choose_language(false, "tw"), "zh-TW");
    }

    #[test]
    fn other_languages_and_regions_use_english() {
        assert_eq!(choose_language(false, "US"), "en");
        assert_eq!(choose_language(false, "JP"), "en");
        assert_eq!(choose_language(false, "CN"), "en");
        assert_eq!(choose_language(false, ""), "en");
    }
}
