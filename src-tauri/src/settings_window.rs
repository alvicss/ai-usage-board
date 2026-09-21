//! 檔案：settings_window.rs
//! 用途：設定視窗的開與關（40「視窗」、43「設定視窗」）：
//!       - 需要時才建立（不常駐，省記憶體）；
//!       - 已經開著時只帶到前面，不開第二個；有指定分頁（右鍵「關於…」）時切到那一頁，
//!         沒指定（⚙ 按鈕、右鍵「設定…」）就維持使用者正在看的頁面與尚未儲存的修改；
//!       - 標題列的亮暗跟隨設定的色系（跟隨 Windows／亮色／暗色）。
//! 誰會用到：commands.rs 的 open_settings／close_settings；settings_commands.rs（儲存後更新標題列亮暗）；
//!           widget_window.rs（共用 window_theme，把色系換成視窗亮暗）。

use tauri::{AppHandle, Emitter, Manager, Theme, WebviewUrl, WebviewWindowBuilder};

/// 設定視窗的識別名稱（capabilities/settings.json 也用這個名稱授權）。
pub const SETTINGS_LABEL: &str = "settings";
/// 設定視窗大小（43「設定視窗」：約 720×560）與最小尺寸。
const WINDOW_WIDTH: f64 = 720.0;
const WINDOW_HEIGHT: f64 = 560.0;
const MIN_WIDTH: f64 = 640.0;
const MIN_HEIGHT: f64 = 500.0;

/// 設定的色系換成視窗的亮暗：跟隨 Windows 時交給系統決定（None）。
/// 設定視窗用在標題列；小工具視窗（widget_window.rs）用來決定玻璃材質的色調。
pub fn window_theme(color_scheme: &str) -> Option<Theme> {
    match color_scheme {
        "light" => Some(Theme::Light),
        "dark" => Some(Theme::Dark),
        _ => None,
    }
}

/// 只接受四個分頁名稱；其他文字一律當成沒指定（避免任意文字被組進網址）。
fn allowed_tab(tab: Option<&str>) -> Option<&'static str> {
    match tab? {
        "general" => Some("general"),
        "appearance" => Some("appearance"),
        "providers" => Some("providers"),
        "about" => Some("about"),
        _ => None,
    }
}

/// 開啟設定視窗。tab 是要顯示的分頁（可省略）；color_scheme 決定標題列亮暗。
pub fn open(app: &AppHandle, tab: Option<&str>, color_scheme: &str) -> Result<(), String> {
    let tab_name = allowed_tab(tab);

    // 已經開著：帶到前面；有指定分頁就請畫面切過去
    if let Some(window) = app.get_webview_window(SETTINGS_LABEL) {
        let _ = window.unminimize();
        window.show().map_err(|error| error.to_string())?;
        window.set_focus().map_err(|error| error.to_string())?;
        if let Some(name) = tab_name {
            let _ = app.emit_to(SETTINGS_LABEL, "settings-open-tab", name);
        }
        return Ok(());
    }

    // 網址帶 ?settings 讓畫面載入設定頁（App.vue 判斷），tab 決定一開始顯示哪一頁
    let first_tab = tab_name.unwrap_or("general");
    let url = WebviewUrl::App(format!("index.html?settings&tab={first_tab}").into());
    WebviewWindowBuilder::new(app, SETTINGS_LABEL, url)
        .title("AI Usage Board")
        .inner_size(WINDOW_WIDTH, WINDOW_HEIGHT)
        .min_inner_size(MIN_WIDTH, MIN_HEIGHT)
        .theme(window_theme(color_scheme))
        .center()
        .build()
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// 設定儲存後：設定視窗開著的話，標題列亮暗跟著新的色系。
pub fn apply_theme(app: &AppHandle, color_scheme: &str) {
    if let Some(window) = app.get_webview_window(SETTINGS_LABEL) {
        let _ = window.set_theme(window_theme(color_scheme));
    }
}

/// 關閉設定視窗（沒開就什麼都不做）。
pub fn close(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(SETTINGS_LABEL) {
        window.close().map_err(|error| error.to_string())?;
    }
    Ok(())
}
