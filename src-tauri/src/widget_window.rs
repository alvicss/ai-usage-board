//! 檔案：widget_window.rs
//! 用途：小工具視窗的桌面行為（FR-14）：
//!       - 啟動時恢復位置大小（第一次開在主螢幕右上角），釘在桌面最下層、不上工作列與 Alt+Tab，再顯示出來；
//!       - 使用者拖曳或縮放結束後，記住目前顯示方式的位置大小（輪播、並排各自記住）；
//!       - ⇄ 切換顯示方式時，換成該模式記住的大小，以右上角為基準；
//!       - 已經在執行時又開一次（單一執行個體）、或按一下系統匣圖示：讓現有的小工具閃一下提示位置；
//!       - 按 Win+D（顯示桌面）時暫時浮上來、切回其他視窗時放回最下層（S-001，偵測在 desktop/show_desktop.rs）；
//!       - 外觀（M-007）：視窗的亮暗跟著色系；玻璃主題、Windows 11 支援時套 Acrylic 材質，
//!         其他情況移除材質（43「玻璃材質」）。
//!       位置怎麼算在 window_placement.rs（純函式）；Win32 專用的部分在 desktop.rs。
//! 誰會用到：lib.rs（啟動、單一執行個體、視窗事件）、commands.rs（save_window_bounds、set_display_mode）、
//!           settings_commands.rs（在設定畫面改了顯示方式、主題、色系）。

use crate::runtime_state::WindowBounds;
use crate::scheduler::Scheduler;
use crate::settings::{Settings, SettingsStore};
use crate::window_placement::{self, ScreenArea};
use crate::{desktop, settings_window};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewWindow};

/// 小工具視窗的識別名稱（tauri.conf.json 與 capabilities/widget.json 也用這個名稱）。
pub const WIDGET_LABEL: &str = "widget";
/// 兩種顯示方式的預設大小（邏輯像素，43「頁面版面配置」）。
const CAROUSEL_DEFAULT_SIZE: (f64, f64) = (360.0, 230.0);
const OVERVIEW_DEFAULT_SIZE: (f64, f64) = (640.0, 240.0);
/// 第一次開啟時距螢幕工作區邊緣的距離（邏輯像素，FR-14）。
const SCREEN_MARGIN: f64 = 24.0;
/// 單一執行個體提示：小工具暫時浮到最上層、閃爍的時間（與畫面端 widget-flash 動畫長度一致）。
const FLASH_DURATION: Duration = Duration::from_millis(1800);

/// 這個顯示方式的預設大小。
fn default_logical_size(mode: &str) -> (f64, f64) {
    if mode == "overview" {
        OVERVIEW_DEFAULT_SIZE
    } else {
        CAROUSEL_DEFAULT_SIZE
    }
}

/// 把 Tauri 的螢幕資訊轉成位置計算用的「工作區」。
fn to_screen_area(monitor: &Monitor) -> ScreenArea {
    let work_area = monitor.work_area();
    ScreenArea {
        x: work_area.position.x,
        y: work_area.position.y,
        width: work_area.size.width,
        height: work_area.size.height,
        scale_factor: monitor.scale_factor(),
    }
}

/// 目前所有螢幕的工作區，以及主螢幕的工作區；一個螢幕都拿不到時回傳 None。
fn screen_areas(window: &WebviewWindow) -> Option<(Vec<ScreenArea>, ScreenArea)> {
    let monitors = window.available_monitors().ok()?;
    let mut areas = Vec::new();
    for monitor in &monitors {
        areas.push(to_screen_area(monitor));
    }

    let primary = match window.primary_monitor() {
        Ok(Some(monitor)) => to_screen_area(&monitor),
        _ => areas.first()?.clone(),
    };
    Some((areas, primary))
}

/// 讀取視窗目前的位置與大小（實體像素）。
fn current_bounds(window: &WebviewWindow) -> Result<WindowBounds, String> {
    let position = window.outer_position().map_err(|error| error.to_string())?;
    let size = window.outer_size().map_err(|error| error.to_string())?;
    Ok(WindowBounds {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    })
}

/// 視窗目前所在螢幕的名稱（例如 \\.\DISPLAY1），只存進 state.json 供除錯參考。
fn current_monitor_name(window: &WebviewWindow) -> String {
    match window.current_monitor() {
        Ok(Some(monitor)) => monitor.name().cloned().unwrap_or_default(),
        _ => String::new(),
    }
}

/// 把位置大小套到視窗上。
/// 先移位置、再設大小：移到縮放比例不同的螢幕時，Windows 會自動依新螢幕的比例縮放視窗，
/// 先移過去再設大小，最後的大小才會是我們要的實體像素（反過來做的話，
/// 從 125% 的主螢幕移到 100% 的副螢幕，360×230 會被縮成 288×184）。
fn apply_bounds(window: &WebviewWindow, bounds: WindowBounds) -> tauri::Result<()> {
    window.set_position(PhysicalPosition::new(bounds.x, bounds.y))?;
    window.set_size(PhysicalSize::new(bounds.width, bounds.height))
}

/// 程式啟動時：恢復位置大小 → 套用外觀（亮暗、玻璃材質）→ 釘在最下層 → 顯示 → 從工作列與 Alt+Tab 移除。
/// 視窗在 tauri.conf.json 設為一開始不顯示，避免先閃一下出現在錯的位置。
pub fn prepare_on_startup(
    app: &AppHandle,
    scheduler: &Scheduler,
    settings: &Settings,
) -> tauri::Result<()> {
    let Some(window) = app.get_webview_window(WIDGET_LABEL) else {
        return Ok(());
    };
    let mode = settings.display.mode.as_str();

    if let Some((areas, primary)) = screen_areas(&window) {
        let saved = scheduler.window_bounds(mode);
        let bounds = window_placement::restore_bounds(
            saved,
            &areas,
            &primary,
            default_logical_size(mode),
            SCREEN_MARGIN,
        );
        apply_bounds(&window, bounds)?;
        // 實際擺放的位置也記下來（例如外接螢幕拔掉後移回主螢幕，下次就從這裡開始）
        if let Err(error) =
            scheduler.save_window_bounds(mode, bounds, &current_monitor_name(&window))
        {
            log::warn!("記住小工具位置失敗：{error}");
        }
        log::info!(
            "小工具位置 ({}, {}) 大小 {}×{}（{}）",
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
            if saved.is_some() {
                "恢復上次位置"
            } else {
                "第一次開啟，放在主螢幕右上角"
            }
        );
    }

    apply_window_appearance(&window, settings);
    window.set_always_on_bottom(true)?;
    window.show()?;
    // 顯示視窗會讓 Tauri 重設延伸樣式，所以放在 show() 之後
    desktop::keep_off_taskbar_and_alt_tab(&window);
    Ok(())
}

/// 玻璃主題要不要套 Acrylic：主題是玻璃、這台 Windows 支援。
pub fn wants_acrylic(settings: &Settings) -> bool {
    if settings.display.theme != "glass" {
        return false;
    }
    desktop::acrylic_supported()
}

/// 設定儲存後：小工具視窗的外觀跟著新設定（亮暗、玻璃材質）。
pub fn apply_appearance(app: &AppHandle, settings: &Settings) {
    let Some(window) = app.get_webview_window(WIDGET_LABEL) else {
        return;
    };
    apply_window_appearance(&window, settings);
}

/// 視窗的亮暗與材質：
/// 1. 亮暗跟著色系（跟隨 Windows 時交給系統）；Acrylic 的色調就是依視窗的亮暗決定。
/// 2. 需要時套 Acrylic（含圓角），不需要時移除，視窗維持完全透明、卡片由 CSS 自己畫。
/// 3. 改亮暗可能讓 Tauri 重設延伸樣式，最後再把小工具從 Alt+Tab 移除一次。
fn apply_window_appearance(window: &WebviewWindow, settings: &Settings) {
    let theme = settings_window::window_theme(&settings.display.color_scheme);
    if let Err(error) = window.set_theme(theme) {
        log::warn!("設定小工具視窗亮暗失敗：{error}");
    }
    desktop::set_acrylic(window, wants_acrylic(settings));
    desktop::keep_off_taskbar_and_alt_tab(window);
}

/// 小工具失去焦點、或 Windows 切換亮暗之後：玻璃材質還在用的話，讓它繼續以毛玻璃顯示
/// （Windows 11 的材質在視窗「不在作用中」時會變成一片灰色，見 desktop.rs 說明）。
pub fn keep_acrylic_active(app: &AppHandle) {
    let Some(window) = app.get_webview_window(WIDGET_LABEL) else {
        return;
    };
    let Some(store) = app.try_state::<SettingsStore>() else {
        return;
    };
    if wants_acrylic(&store.current()) {
        desktop::keep_acrylic_active(&window);
    }
}

/// 使用者拖曳或縮放結束：記住目前顯示方式的位置大小。
pub fn remember_current_bounds(
    app: &AppHandle,
    scheduler: &Scheduler,
    mode: &str,
) -> Result<(), String> {
    let Some(window) = app.get_webview_window(WIDGET_LABEL) else {
        return Ok(());
    };
    let bounds = current_bounds(&window)?;
    scheduler.save_window_bounds(mode, bounds, &current_monitor_name(&window))
}

/// 切換顯示方式之後：換成新模式記住的大小（沒記過就用預設大小），以右上角為基準，超出工作區就往內移。
pub fn apply_mode_size(app: &AppHandle, scheduler: &Scheduler, mode: &str) -> Result<(), String> {
    let Some(window) = app.get_webview_window(WIDGET_LABEL) else {
        return Ok(());
    };
    let Some((areas, primary)) = screen_areas(&window) else {
        return Ok(());
    };
    let current = current_bounds(&window)?;

    let new_size = match scheduler.window_bounds(mode) {
        Some(saved) => (saved.width, saved.height),
        None => {
            // 沒記過：用預設大小，依視窗目前所在螢幕的縮放比例換算
            let area =
                window_placement::area_containing_center(&current, &areas).unwrap_or(&primary);
            window_placement::physical_size(default_logical_size(mode), area.scale_factor)
        }
    };

    let bounds = window_placement::switch_mode_bounds(current, new_size, &areas, &primary);
    apply_bounds(&window, bounds).map_err(|error| error.to_string())?;
    scheduler.save_window_bounds(mode, bounds, &current_monitor_name(&window))
}

/// 讓現有的小工具浮到最上層閃一下提示位置，再放回原本那一層。
/// 用在兩個地方：已經在執行時又開一次程式（單一執行個體）、按一下系統匣圖示（tray.rs）；reason 寫進 log。
/// 整段放在背景執行緒：這個函式是在主執行緒處理視窗訊息的途中被呼叫的，視窗操作改由主執行緒之後依序處理。
pub fn flash(app: &AppHandle, reason: &str) {
    let Some(window) = app.get_webview_window(WIDGET_LABEL) else {
        return;
    };
    log::info!("{reason}：提示現有小工具的位置");

    thread::spawn(move || {
        desktop::move_to_layer(&window, true);
        desktop::keep_off_taskbar_and_alt_tab(&window);
        let _ = window.emit_to(WIDGET_LABEL, "widget-flash", ());

        // 閃完之後放回原本那一層：正在「顯示桌面」（S-001 浮上來）時維持在上面，否則放回最下層
        thread::sleep(FLASH_DURATION);
        desktop::move_to_layer(&window, desktop::show_desktop_raised());
        desktop::keep_off_taskbar_and_alt_tab(&window);
    });
}

/// S-001：開始監聽「顯示桌面」（Win+D）。偵測到時小工具暫時浮上來，使用者切回其他視窗時放回最下層。
/// 偵測在 desktop/show_desktop.rs（專用背景執行緒）、換層在 desktop/layer.rs；這裡只負責把兩邊接起來。
pub fn start_show_desktop_watcher(app: &AppHandle) {
    let Some(window) = app.get_webview_window(WIDGET_LABEL) else {
        return;
    };
    let app_handle = app.clone();
    let on_change: desktop::LayerCallback = Box::new(move |raised| {
        set_show_desktop_layer(&app_handle, raised);
    });
    desktop::start_show_desktop_watcher(&window, on_change);
}

/// 顯示桌面時浮到最上層（raised = true）、切回其他視窗時放回最下層（false）。
/// 在監聽用的背景執行緒上呼叫；Tauri 會把視窗操作排到主執行緒依序處理。
/// 換層後補回工具視窗樣式（不上 Alt+Tab），玻璃主題再把材質標成作用中（desktop/layer.rs 的 switch_layer）。
fn set_show_desktop_layer(app: &AppHandle, raised: bool) {
    let Some(window) = app.get_webview_window(WIDGET_LABEL) else {
        return;
    };
    let keep_glass_active = match app.try_state::<SettingsStore>() {
        Some(store) => wants_acrylic(&store.current()),
        None => false,
    };
    desktop::switch_layer(&window, raised, keep_glass_active);
}
