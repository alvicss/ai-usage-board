//! 檔案：reset_credits_window.rs
//! 用途：重置券小視窗（M-014，43「重置券視窗」）的開與關：
//!       - 小工具右上角的票券圖示：沒開就開、開著就關；
//!       - 無邊框、透明，跟小工具同主題（畫面由 src/views/ResetCreditsView.vue 畫），開在小工具旁邊
//!         （位置怎麼算在 window_placement.rs 的 popup_beside_bounds）；
//!       - 先建立「看不見」的視窗，畫面畫好、量出內容高度後呼叫 reveal 才調整大小並顯示，避免閃一下或大小不對；
//!       - 點別的地方（失去焦點）就自動關；也可以按 ✕ 或 Esc（畫面呼叫 close）；
//!       - 玻璃主題、Windows 11 支援時套 Acrylic；小視窗開著時一定是作用中視窗，毛玻璃不會變灰。
//! 誰會用到：commands.rs（open_reset_credits、reveal_reset_credits、close_reset_credits）、lib.rs（視窗事件）。

use crate::settings::Settings;
use crate::window_placement::{self, ScreenArea};
use crate::{desktop, settings_window, widget_window};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder, WindowEvent};

/// 重置券小視窗的識別名稱（capabilities/reset-credits.json 也用這個名稱授權）。
pub const RESET_CREDITS_LABEL: &str = "reset-credits";
/// 視窗寬度（邏輯像素）；高度由畫面量出內容後決定，限制在最小～最大之間（內容更多時畫面內捲動）。
const WINDOW_WIDTH: f64 = 300.0;
const INITIAL_HEIGHT: f64 = 160.0;
const MIN_HEIGHT: f64 = 90.0;
const MAX_HEIGHT: f64 = 420.0;
/// 和小工具之間的距離（邏輯像素）。
const GAP_TO_WIDGET: f64 = 8.0;
/// 小視窗因為失去焦點而關閉後，這段時間內又收到「開啟」：視為使用者是再按一次票券圖示要把它關掉
/// （按小工具上的圖示時，小視窗會先失去焦點被關掉，接著才收到按鈕的開啟要求）。
const REOPEN_GUARD: Duration = Duration::from_millis(400);

/// 畫面沒有在這段時間內回報內容高度時，Rust 自己用預設高度顯示（保險：避免小視窗一直藏著、按了沒反應）。
const REVEAL_FALLBACK_DELAY: Duration = Duration::from_millis(1500);

/// 上一次因為失去焦點而關閉的時間。
static LAST_CLOSED_BY_FOCUS_LOSS: Mutex<Option<Instant>> = Mutex::new(None);
/// 這次開啟的小視窗是否已經顯示出來。顯示之前（建立中、畫面還在載入）收到的焦點變化一律不算數：
/// 2026-11-14 AJ 實測第一版「按了沒跳出來」，log 顯示小視窗從來沒走到顯示那一步，
/// 推測是建立時的焦點變化被當成「點了別的地方」，在顯示前就被關掉。
static REVEALED: AtomicBool = AtomicBool::new(false);
/// 顯示之後是否拿到過焦點；拿到過、之後又失去焦點，才代表使用者點了別的地方。
static HAS_BEEN_FOCUSED: AtomicBool = AtomicBool::new(false);

/// 票券圖示：小視窗沒開就建立（先不顯示、也不搶焦點，等畫面呼叫 reveal），開著就關閉。
pub fn toggle(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    // 開著：關閉
    if app.get_webview_window(RESET_CREDITS_LABEL).is_some() {
        log::info!("重置券視窗：再按一次票券圖示，關閉");
        return close(app);
    }

    // 剛剛才因為點了圖示（失去焦點）被關掉：這次按圖示的意思是「關」，不再打開
    if just_closed_by_focus_loss() {
        return Ok(());
    }

    REVEALED.store(false, Ordering::SeqCst);
    HAS_BEEN_FOCUSED.store(false, Ordering::SeqCst);
    let url = WebviewUrl::App("index.html?reset-credits".into());
    WebviewWindowBuilder::new(app, RESET_CREDITS_LABEL, url)
        .title("AI Usage Board")
        .inner_size(WINDOW_WIDTH, INITIAL_HEIGHT)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .theme(settings_window::window_theme(&settings.display.color_scheme))
        .visible(false)
        // 藏著的時候不要拿焦點，顯示時（reveal）才拿
        .focused(false)
        .build()
        .map_err(|error| log_failure("建立", error))?;
    log::info!("重置券視窗：建立（等畫面回報內容高度後顯示）");

    // 保險：畫面一段時間內沒回報高度（例如載入出錯），就用預設高度直接顯示
    let fallback_app = app.clone();
    let fallback_settings = settings.clone();
    std::thread::spawn(move || {
        std::thread::sleep(REVEAL_FALLBACK_DELAY);
        if REVEALED.load(Ordering::SeqCst) || fallback_app.get_webview_window(RESET_CREDITS_LABEL).is_none() {
            return;
        }
        log::warn!("重置券視窗：畫面 1.5 秒內沒有回報內容高度，改用預設高度顯示");
        if let Err(error) = reveal(&fallback_app, &fallback_settings, INITIAL_HEIGHT, None) {
            log::warn!("重置券視窗：用預設高度顯示也失敗：{error}");
        }
    });
    Ok(())
}

/// 畫面畫好後：依內容高度調整大小、放到小工具旁邊、套玻璃材質，再顯示並取得焦點。
/// 資料更新、內容高度或像素比例改變時畫面也會再呼叫一次（只調整大小與位置）。
/// 輸入：content_height 是畫面量到的內容高度（CSS px）；pixel_ratio 是畫面的 devicePixelRatio
///       （1 個 CSS px 等於幾個螢幕實體像素）。
/// 為什麼要畫面給 pixel_ratio：WebView2 除了 Windows 顯示縮放，還會套用「文字大小」等放大設定，
/// 2026-11-14 實測 AJ 這台螢幕縮放 125%、畫面實際是 1.3125；只用螢幕縮放換算，視窗會比內容矮、跑出捲軸。
/// pixel_ratio 為 None（Rust 自己的保險顯示）或數值不合理時，改用小工具所在螢幕的縮放比例。
pub fn reveal(
    app: &AppHandle,
    settings: &Settings,
    content_height: f64,
    pixel_ratio: Option<f64>,
) -> Result<(), String> {
    let Some(window) = app.get_webview_window(RESET_CREDITS_LABEL) else {
        log::warn!("重置券視窗：要顯示時視窗已經不在（可能在顯示前就被關掉）");
        return Ok(());
    };
    let height = content_height.clamp(MIN_HEIGHT, MAX_HEIGHT);

    // 位置：開在小工具旁邊（讀不到小工具時就留在原地）
    if let Some((widget, areas, primary)) = widget_window::bounds_and_screens(app) {
        let scale_factor = match pixel_ratio {
            Some(ratio) if ratio.is_finite() && ratio > 0.0 => ratio,
            _ => scale_of_widget_screen(&widget, &areas, &primary),
        };
        let size = window_placement::physical_size((WINDOW_WIDTH, height), scale_factor);
        let bounds = window_placement::popup_beside_bounds(&widget, size, &areas, &primary, GAP_TO_WIDGET);
        // 先移位置、再設大小：移到縮放比例不同的螢幕時，大小才不會被 Windows 自動縮放（22 踩過的坑）
        window
            .set_position(PhysicalPosition::new(bounds.x, bounds.y))
            .map_err(|error| log_failure("移動", error))?;
        window
            .set_size(PhysicalSize::new(bounds.width, bounds.height))
            .map_err(|error| log_failure("調整大小", error))?;
    }

    // 從這裡開始，焦點變化才算數（點別的地方就關閉）
    let first_reveal = !REVEALED.swap(true, Ordering::SeqCst);
    desktop::set_acrylic(&window, widget_window::wants_acrylic(settings));
    window.show().map_err(|error| log_failure("顯示", error))?;
    window.set_focus().map_err(|error| log_failure("取得焦點", error))?;
    // 顯示視窗會讓 Tauri 重設延伸樣式，最後再把小視窗從 Alt+Tab 移除
    desktop::keep_off_taskbar_and_alt_tab(&window);
    if first_reveal {
        log::info!("重置券視窗：顯示（內容高度 {height:.0}）");
    }
    Ok(())
}

/// 關閉小視窗（沒開就什麼都不做）：✕ 按鈕、Esc、再按一次票券圖示。
/// 關閉前先清掉記號：視窗關掉時 Windows 也會送一次「失去焦點」，
/// 不清掉的話會被當成「點了別的地方」，緊接著按票券圖示就打不開。
pub fn close(app: &AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(RESET_CREDITS_LABEL) {
        REVEALED.store(false, Ordering::SeqCst);
        HAS_BEEN_FOCUSED.store(false, Ordering::SeqCst);
        window.close().map_err(|error| log_failure("關閉", error))?;
    }
    Ok(())
}

/// 小視窗的視窗事件（lib.rs 轉過來）：顯示之後拿到焦點時記下來；之後失去焦點（點了別的地方）就關閉。
/// 關閉放到背景工作再發出，由主執行緒依序處理，不在視窗事件處理途中直接關視窗（22 踩過的坑）。
pub fn on_window_event(app: &AppHandle, event: &WindowEvent) {
    // 還沒顯示（建立中、畫面載入中）的焦點變化不算數
    if !REVEALED.load(Ordering::SeqCst) {
        return;
    }
    match event {
        WindowEvent::Focused(true) => {
            HAS_BEEN_FOCUSED.store(true, Ordering::SeqCst);
        }
        WindowEvent::Focused(false) => {
            if !HAS_BEEN_FOCUSED.swap(false, Ordering::SeqCst) {
                return;
            }
            log::info!("重置券視窗：點了別的地方，關閉");
            remember_focus_loss_close();
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = close(&app) {
                    log::warn!("關閉重置券視窗失敗：{error}");
                }
            });
        }
        _ => {}
    }
}

/// 小工具所在螢幕的縮放比例（找不到時用主螢幕的）。
fn scale_of_widget_screen(
    widget: &crate::runtime_state::WindowBounds,
    areas: &[ScreenArea],
    primary: &ScreenArea,
) -> f64 {
    match window_placement::area_containing_center(widget, areas) {
        Some(area) => area.scale_factor,
        None => primary.scale_factor,
    }
}

/// 某個步驟失敗：寫一筆 log（方便從 log 查出卡在哪一步），回傳給畫面的錯誤文字。
fn log_failure(step: &str, error: impl std::fmt::Display) -> String {
    log::warn!("重置券視窗：{step}失敗：{error}");
    error.to_string()
}

/// 記下「因為失去焦點而關閉」的時間。
fn remember_focus_loss_close() {
    if let Ok(mut last) = LAST_CLOSED_BY_FOCUS_LOSS.lock() {
        *last = Some(Instant::now());
    }
}

/// 是否剛剛（REOPEN_GUARD 之內）才因為失去焦點而關閉；查過就清掉，下一次按圖示就會正常打開。
fn just_closed_by_focus_loss() -> bool {
    let Ok(mut last) = LAST_CLOSED_BY_FOCUS_LOSS.lock() else {
        return false;
    };
    let recently = matches!(*last, Some(closed_at) if closed_at.elapsed() < REOPEN_GUARD);
    *last = None;
    recently
}
