//! 檔案：tray.rs
//! 用途：Windows 右下角通知區（系統匣）的本程式圖示（M-010、21 FR-19）。小工具右上角沒有「╳」，
//!       使用者可從工作列右下角關掉程式：
//!       - 圖示用程式圖示（與 exe 相同），滑鼠停在上面顯示「AI Usage Board」；
//!       - 右鍵選單和小工具的右鍵選單一樣：立即更新／切換顯示方式／設定…／關於…／（分隔線）／結束；
//!         選單文字直接取自畫面的翻譯檔（src/i18n/*.json 的 app.*），兩邊不會不一致，並跟著語言設定切換；
//!       - 左鍵按一下：小工具浮上來閃一下提示位置（和重複開啟程式時相同，widget_window::flash）；
//!       - 「結束」和小工具右鍵的「結束」相同：照常結束程式（查詢中的子行程由 Job Object 一起收掉，
//!         系統匣圖示由 Tauri 在結束前移除）。
//!       系統匣只在 Rust 端建立與處理，畫面端沒有系統匣權限。
//!       選單的動作放到背景執行緒執行：選單事件是在主執行緒處理視窗訊息的途中送來的，
//!       直接在這裡開關、縮放視窗可能觸發 tao 的斷言（22 踩過的坑），和畫面按鈕走 async 指令的道理相同。
//! 誰會用到：lib.rs（啟動時建立）、settings_commands.rs（儲存設定後換語言）。

use crate::scheduler::Scheduler;
use crate::settings::SettingsStore;
use crate::{commands, settings_window, system_info, widget_window};
use serde_json::Value;
use std::thread;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

/// 系統匣圖示的識別名稱（程式只有這一個圖示）。
const TRAY_ID: &str = "main";
/// 滑鼠停在圖示上的提示文字（程式名稱，不翻譯）。
const TOOLTIP: &str = "AI Usage Board";
/// 畫面的翻譯檔（編譯時嵌入），選單文字從這裡取，和小工具右鍵選單一字不差。
const ZH_TW_TEXTS: &str = include_str!("../../src/i18n/zh-TW.json");
const EN_TEXTS: &str = include_str!("../../src/i18n/en.json");

/// 選單項目的 ID。前面加 tray- ：Tauri 的選單事件是全程式共用的，
/// 小工具右鍵選單的項目 ID（refresh、mode…）不能和這裡重複，否則按一次會做兩次。
const ITEM_REFRESH: &str = "tray-refresh";
const ITEM_SWITCH_MODE: &str = "tray-switch-mode";
const ITEM_SETTINGS: &str = "tray-settings";
const ITEM_ABOUT: &str = "tray-about";
const ITEM_QUIT: &str = "tray-quit";

/// 系統匣選單的五個文字（依語言）。
#[derive(Debug, Clone, PartialEq)]
pub struct TrayLabels {
    pub refresh: String,
    pub switch_mode: String,
    pub settings: String,
    pub about: String,
    pub quit: String,
}

/// 依語言（"zh-TW" 或 "en"）取出選單文字；翻譯檔裡找不到的項目用 key 本身代替（不會讓程式當掉）。
pub fn tray_labels(language: &str) -> TrayLabels {
    let source = if language == "zh-TW" {
        ZH_TW_TEXTS
    } else {
        EN_TEXTS
    };
    let texts: Value = serde_json::from_str(source).unwrap_or(Value::Null);
    TrayLabels {
        refresh: text_of(&texts, "refresh"),
        switch_mode: text_of(&texts, "switch_mode"),
        settings: text_of(&texts, "menu_settings"),
        about: text_of(&texts, "menu_about"),
        quit: text_of(&texts, "quit"),
    }
}

/// 翻譯檔 app.<key> 的文字；沒有的話回傳 key。
fn text_of(texts: &Value, key: &str) -> String {
    let pointer = format!("/app/{key}");
    match texts.pointer(&pointer).and_then(Value::as_str) {
        Some(text) => text.to_string(),
        None => key.to_string(),
    }
}

/// 建立系統匣的右鍵選單（順序與小工具右鍵選單相同）。
fn build_menu(app: &AppHandle, language: &str) -> tauri::Result<Menu<tauri::Wry>> {
    let labels = tray_labels(language);
    let refresh = MenuItem::with_id(app, ITEM_REFRESH, &labels.refresh, true, None::<&str>)?;
    let switch_mode =
        MenuItem::with_id(app, ITEM_SWITCH_MODE, &labels.switch_mode, true, None::<&str>)?;
    let settings = MenuItem::with_id(app, ITEM_SETTINGS, &labels.settings, true, None::<&str>)?;
    let about = MenuItem::with_id(app, ITEM_ABOUT, &labels.about, true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, ITEM_QUIT, &labels.quit, true, None::<&str>)?;
    Menu::with_items(
        app,
        &[&refresh, &switch_mode, &settings, &about, &separator, &quit],
    )
}

/// 程式啟動時建立系統匣圖示。language_setting 是設定檔的語言（auto／zh-TW／en）。
/// 建立失敗只寫 log，不影響小工具本身。
pub fn create(app: &AppHandle, language_setting: &str) {
    let language = system_info::resolve_language(language_setting);
    let menu = match build_menu(app, language) {
        Ok(menu) => menu,
        Err(error) => {
            log::warn!("無法建立系統匣選單：{error}");
            return;
        }
    };

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip(TOOLTIP)
        .menu(&menu)
        // 左鍵留給「小工具閃一下」，右鍵才出選單
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(on_tray_icon_event);
    // 圖示用程式圖示（tauri.conf.json 的 bundle.icon，編譯時嵌入）
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }

    match builder.build(app) {
        Ok(_) => log::info!("系統匣圖示已建立"),
        Err(error) => log::warn!("無法建立系統匣圖示：{error}"),
    }
}

/// 語言設定改了之後，系統匣選單換成新語言（language_setting：auto／zh-TW／en）。
pub fn apply_language(app: &AppHandle, language_setting: &str) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    let language = system_info::resolve_language(language_setting);
    match build_menu(app, language) {
        Ok(menu) => {
            if let Err(error) = tray.set_menu(Some(menu)) {
                log::warn!("系統匣選單換語言失敗：{error}");
            }
        }
        Err(error) => log::warn!("無法建立系統匣選單：{error}"),
    }
}

/// 選單被點了（所有選單的事件都會送到這裡，只處理 tray- 開頭的項目）。
fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    let item_id = event.id().as_ref().to_string();
    if !item_id.starts_with("tray-") {
        return;
    }
    // 放到背景執行緒做：不要在主執行緒處理選單訊息的途中開關、縮放視窗
    let app_handle = app.clone();
    thread::spawn(move || run_menu_item(&app_handle, &item_id));
}

/// 執行選單項目（在背景執行緒上；視窗操作由 Tauri 排到主執行緒依序處理）。
fn run_menu_item(app: &AppHandle, item_id: &str) {
    match item_id {
        ITEM_REFRESH => {
            if let Some(scheduler) = app.try_state::<Scheduler>() {
                scheduler.request_refresh(app.clone(), "系統匣：立即更新");
            }
        }
        ITEM_SWITCH_MODE => switch_display_mode(app),
        ITEM_SETTINGS => open_settings(app, None),
        ITEM_ABOUT => open_settings(app, Some("about")),
        ITEM_QUIT => {
            log::info!("使用者從系統匣結束程式");
            app.exit(0);
        }
        _ => {}
    }
}

/// 「切換顯示方式」：和小工具的 ⇄ 按鈕相同（輪播 ↔ 並排）。
fn switch_display_mode(app: &AppHandle) {
    let (Some(store), Some(scheduler)) =
        (app.try_state::<SettingsStore>(), app.try_state::<Scheduler>())
    else {
        return;
    };
    let next_mode = if store.current().display.mode == "carousel" {
        "overview"
    } else {
        "carousel"
    };
    if let Err(error) = commands::change_display_mode(app, &store, &scheduler, next_mode) {
        log::warn!("系統匣切換顯示方式失敗：{error}");
    }
}

/// 「設定…」「關於…」：開設定視窗（已開著時帶到前面；關於會切到關於頁）。
fn open_settings(app: &AppHandle, tab: Option<&str>) {
    let color_scheme = match app.try_state::<SettingsStore>() {
        Some(store) => store.current().display.color_scheme,
        None => "auto".to_string(),
    };
    if let Err(error) = settings_window::open(app, tab, &color_scheme) {
        log::warn!("系統匣開啟設定視窗失敗：{error}");
    }
}

/// 圖示本身被點了：左鍵放開時讓小工具浮上來閃一下（右鍵由 Tauri 彈出選單）。
fn on_tray_icon_event(tray: &TrayIcon, event: TrayIconEvent) {
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        widget_window::flash(tray.app_handle(), "按了系統匣圖示");
    }
}

#[cfg(test)]
mod tests {
    use super::tray_labels;

    #[test]
    fn labels_follow_the_screen_translation_files() {
        let chinese = tray_labels("zh-TW");
        assert_eq!(chinese.refresh, "立即更新");
        assert_eq!(chinese.switch_mode, "切換顯示方式");
        assert_eq!(chinese.settings, "設定…");
        assert_eq!(chinese.about, "關於…");
        assert_eq!(chinese.quit, "結束");

        let english = tray_labels("en");
        assert_eq!(english.quit, "Quit");
        assert_ne!(english.refresh, "refresh", "英文翻譯檔要有這個 key");
        assert_ne!(english.switch_mode, "switch_mode");
        assert_ne!(english.settings, "menu_settings");
        assert_ne!(english.about, "menu_about");
    }
}
