//! 檔案：desktop.rs
//! 用途：Windows 專用的程式。所有 Win32 API 呼叫集中在這個檔案與子資料夾 desktop/，日後做 Linux 版只需替換這裡（40「視窗」）。
//!       目前負責四件事：
//!       1. 讓小工具不出現在工作列與 Alt+Tab 清單（M-005）。
//!       2. 玻璃主題的 Windows 11 Acrylic 材質：套用／移除、視窗圓角、讓材質一直以「作用中」的樣子顯示（M-007）。
//!       3. 讀 Windows 的顯示語言與地區，給「語言：自動」判斷用（M-008，判斷規則在 system_info.rs）。
//!       4. 按 Win+D（顯示桌面）時讓小工具暫時浮上來（S-001）：偵測在 desktop/show_desktop.rs，
//!          判斷規則與換層（浮上來、放回最下層、補回工具視窗樣式與玻璃作用中）在 desktop/layer.rs。
//! 誰會用到：widget_window.rs（啟動、切換最上層／最下層、套用外觀、顯示桌面）、lib.rs（視窗事件）、system_info.rs（語言與地區）。
//!
//! 為什麼需要「不上 Alt+Tab」：Tauri 的 skipTaskbar 只把工作列按鈕拿掉，Alt+Tab 仍看得到小工具；
//! Windows 的做法是在視窗加上「工具視窗」延伸樣式（WS_EX_TOOLWINDOW）並拿掉 WS_EX_APPWINDOW。
//! 注意：Tauri（tao）每次改變視窗旗標（例如顯示視窗、設定最上層／最下層）都會重設延伸樣式，
//! 所以在那些操作之後都要再呼叫一次 keep_off_taskbar_and_alt_tab()。
//!
//! 為什麼 Acrylic 要「一直作用中」：Windows 11 的系統材質只有在視窗是「作用中」時才畫毛玻璃，
//! 不在作用中就換成一片灰色。小工具在桌面最下層、平常不會被點到，因此容易顯示成灰色；
//! 在套用材質、視窗失去焦點、亮暗切換之後，直接告訴 Windows「這個視窗的外框是作用中」（WM_NCACTIVATE），
//! 毛玻璃就會出現。這只影響材質的畫法，不會搶焦點，也不會改變哪個視窗在前面。

mod layer;
mod show_desktop;

pub use layer::{move_to_layer, switch_layer, LayerCallback};

use tauri::WebviewWindow;

/// 開始監聽「顯示桌面」（Win+D，S-001）：偵測到時呼叫 on_change(true) 讓小工具浮上來，
/// 使用者切回其他視窗時呼叫 on_change(false) 放回最下層。on_change 在監聽用的背景執行緒上執行。
#[cfg(windows)]
pub fn start_show_desktop_watcher(window: &WebviewWindow, on_change: LayerCallback) {
    match window.hwnd() {
        Ok(hwnd) => show_desktop::start(hwnd.0 as isize, on_change),
        Err(error) => log::warn!("取得小工具視窗代碼失敗，Win+D 時小工具不會浮上來：{error}"),
    }
}

/// 其他作業系統：沒有 Win+D，不需要監聽。
#[cfg(not(windows))]
pub fn start_show_desktop_watcher(_window: &WebviewWindow, _on_change: LayerCallback) {}

/// 小工具目前是不是因為「顯示桌面」浮在最上層。
pub fn show_desktop_raised() -> bool {
    layer::is_raised()
}

/// 讓視窗不出現在工作列與 Alt+Tab。
/// 排到主執行緒上執行：Tauri 的視窗操作也是在主執行緒依序處理，這樣一定排在前面的操作之後，
/// 不會被它們重設的樣式蓋掉。失敗只寫 log，不影響其他功能。
pub fn keep_off_taskbar_and_alt_tab(window: &WebviewWindow) {
    let target = window.clone();
    let result = window.run_on_main_thread(move || apply_tool_window_style(&target));
    if let Err(error) = result {
        log::warn!("無法把小工具從 Alt+Tab 清單移除：{error}");
    }
}

/// 真正修改延伸樣式（Windows）。
#[cfg(windows)]
fn apply_tool_window_style(window: &WebviewWindow) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, SWP_FRAMECHANGED,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
    };

    let hwnd = match window.hwnd() {
        Ok(hwnd) => hwnd,
        Err(error) => {
            log::warn!("取得小工具視窗代碼失敗：{error}");
            return;
        }
    };

    // SAFETY：hwnd 來自 Tauri 目前仍存在的視窗；只讀寫它自己的延伸樣式，不涉及記憶體所有權。
    unsafe {
        let old_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let tool_window_bit = WS_EX_TOOLWINDOW.0 as isize;
        let app_window_bit = WS_EX_APPWINDOW.0 as isize;

        // 加上「工具視窗」、拿掉「應用程式視窗」
        let mut new_style = old_style | tool_window_bit;
        new_style &= !app_window_bit;
        if new_style == old_style {
            return;
        }

        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_style);
        // 樣式改變後要通知 Windows 重新套用（不移動、不改大小、不改前後順序、不搶焦點）
        let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED;
        if let Err(error) = SetWindowPos(hwnd, None, 0, 0, 0, 0, flags) {
            log::warn!("套用工具視窗樣式失敗：{error}");
        }
    }
}

/// 其他作業系統：沒有 Alt+Tab 的概念，不需要處理。
#[cfg(not(windows))]
fn apply_tool_window_style(_window: &WebviewWindow) {}

// ---------------------------------------------------------------------------
// 玻璃主題的 Acrylic 材質（43「玻璃材質」）
// ---------------------------------------------------------------------------

/// Windows 11 從這個版本（22H2，組建 22621；測試版 22523 起）開始提供「系統 Acrylic 材質」。
/// 更舊的 Windows（Windows 10、Windows 11 21H2）沒有，玻璃主題改用半透明底（畫面端的 CSS 處理）。
const SYSTEM_ACRYLIC_MIN_BUILD: u32 = 22523;

/// 這台電腦能不能用系統 Acrylic 材質。
#[cfg(windows)]
pub fn acrylic_supported() -> bool {
    let version = windows_version::OsVersion::current();
    version.major >= 10 && version.build >= SYSTEM_ACRYLIC_MIN_BUILD
}

/// 其他作業系統：沒有 Acrylic。
#[cfg(not(windows))]
pub fn acrylic_supported() -> bool {
    false
}

/// 套用（enabled = true）或移除（false）小工具視窗的 Acrylic 材質。
/// 套用時一併：視窗四角改圓角（Windows 11 的標準圓角，8 邏輯像素）、不畫 Windows 的視窗外框線
/// （外框由卡片的 CSS 邊框負責，FR-18 的邊框自訂才有效），並讓材質以作用中的樣子顯示。
/// 移除時把這三項都還原成 Windows 預設值。排到主執行緒執行，排在前面的視窗操作之後。
pub fn set_acrylic(window: &WebviewWindow, enabled: bool) {
    let target = window.clone();
    let result = window.run_on_main_thread(move || apply_acrylic(&target, enabled));
    if let Err(error) = result {
        log::warn!("無法切換玻璃材質：{error}");
    }
}

/// 材質已經套用時，再告訴 Windows 一次「外框是作用中」，毛玻璃才不會變回一片灰色。
/// 在小工具失去焦點、亮暗切換之後呼叫（lib.rs 的視窗事件）。
pub fn keep_acrylic_active(window: &WebviewWindow) {
    let target = window.clone();
    let result = window.run_on_main_thread(move || mark_frame_active(&target));
    if let Err(error) = result {
        log::warn!("無法維持玻璃材質：{error}");
    }
}

/// 真正設定材質（Windows）。
#[cfg(windows)]
fn apply_acrylic(window: &WebviewWindow, enabled: bool) {
    use windows::Win32::Graphics::Dwm::{
        DWMSBT_NONE, DWMSBT_TRANSIENTWINDOW, DWMWA_BORDER_COLOR, DWMWA_COLOR_DEFAULT,
        DWMWA_COLOR_NONE, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_WINDOW_CORNER_PREFERENCE,
        DWMWCP_DEFAULT, DWMWCP_ROUND,
    };

    // 沒有系統 Acrylic 的 Windows（Windows 10 等）：從來不會套材質，也就不用移除
    if !acrylic_supported() {
        return;
    }
    let hwnd = match window.hwnd() {
        Ok(hwnd) => hwnd,
        Err(error) => {
            log::warn!("取得小工具視窗代碼失敗：{error}");
            return;
        }
    };

    // 依「套用／移除」決定三個屬性的值：材質、圓角、外框線顏色
    let (backdrop, corner, border_color) = if enabled {
        // Acrylic（Windows 稱為「暫時性視窗」材質）、Windows 11 標準圓角、不畫外框線
        (DWMSBT_TRANSIENTWINDOW.0, DWMWCP_ROUND.0, DWMWA_COLOR_NONE)
    } else {
        // 全部還原成 Windows 預設值
        (DWMSBT_NONE.0, DWMWCP_DEFAULT.0, DWMWA_COLOR_DEFAULT)
    };

    set_dwm_attribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, backdrop as u32, "材質");
    set_dwm_attribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, corner as u32, "圓角");
    set_dwm_attribute(hwnd, DWMWA_BORDER_COLOR, border_color, "外框線");

    if enabled {
        mark_frame_active(window);
    }
    log::info!("玻璃材質：{}", if enabled { "套用 Acrylic" } else { "移除" });
}

/// 其他作業系統：沒有材質可以套。
#[cfg(not(windows))]
fn apply_acrylic(_window: &WebviewWindow, _enabled: bool) {}

/// 設定一個 DWM 視窗屬性（值都是 4 位元組的整數）；失敗只寫 log。
#[cfg(windows)]
fn set_dwm_attribute(
    hwnd: windows::Win32::Foundation::HWND,
    attribute: windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE,
    value: u32,
    name: &str,
) {
    use windows::Win32::Graphics::Dwm::DwmSetWindowAttribute;

    let value_pointer = &value as *const u32 as *const core::ffi::c_void;
    // SAFETY：hwnd 是 Tauri 目前仍存在的視窗；value 在呼叫期間有效，大小固定 4 位元組。
    let result = unsafe { DwmSetWindowAttribute(hwnd, attribute, value_pointer, 4) };
    if let Err(error) = result {
        log::warn!("設定視窗{name}失敗：{error}");
    }
}

/// 告訴 Windows「這個視窗的外框是作用中」，系統材質就會畫出毛玻璃（不是一片灰色）。
/// 直接交給 Windows 預設的處理程序，不經過 Tauri，所以不會讓 Tauri 以為視窗拿到了焦點。
#[cfg(windows)]
fn mark_frame_active(window: &WebviewWindow) {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{DefWindowProcW, WM_NCACTIVATE};

    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    // SAFETY：hwnd 是 Tauri 目前仍存在的視窗；WM_NCACTIVATE 只影響外框的畫法。
    unsafe {
        DefWindowProcW(hwnd, WM_NCACTIVATE, WPARAM(1), LPARAM(0));
    }
}

/// 其他作業系統：不需要處理。
#[cfg(not(windows))]
fn mark_frame_active(_window: &WebviewWindow) {}

// ---------------------------------------------------------------------------
// Windows 顯示語言與地區（FR-17「語言：自動」）
// ---------------------------------------------------------------------------

/// Windows 的顯示語言（使用者介面語言）是不是中文（繁體、簡體都算）。
#[cfg(windows)]
pub fn ui_language_is_chinese() -> bool {
    use windows::Win32::Globalization::GetUserDefaultUILanguage;

    /// 語言代碼的主要語言部分（低 10 位元）是 0x04 就是中文
    const PRIMARY_LANGUAGE_MASK: u16 = 0x03FF;
    const LANG_CHINESE: u16 = 0x0004;

    // SAFETY：不需要任何參數，只讀取目前使用者的設定。
    let language_id = unsafe { GetUserDefaultUILanguage() };
    language_id & PRIMARY_LANGUAGE_MASK == LANG_CHINESE
}

/// 其他作業系統：先當作不是中文（M-009 之後做其他平台時再補）。
#[cfg(not(windows))]
pub fn ui_language_is_chinese() -> bool {
    false
}

/// Windows 設定的「國家或地區」（Home location）的兩碼代號，例如 TW、HK、US；讀不到時回傳空字串。
#[cfg(windows)]
pub fn home_region_code() -> String {
    use windows::Win32::Globalization::{GetGeoInfoW, GetUserGeoID, GEOCLASS_NATION, GEO_ISO2};

    // SAFETY：只讀取目前使用者的地區設定。
    let geo_id = unsafe { GetUserGeoID(GEOCLASS_NATION) };
    if geo_id < 0 {
        return String::new();
    }

    // 地區編號換成兩碼代號；buffer 足夠放兩個字母加結尾
    let mut buffer = [0u16; 8];
    // SAFETY：buffer 是本函式的區域陣列，長度由切片自己帶給 Windows。
    let written = unsafe { GetGeoInfoW(geo_id, GEO_ISO2, Some(&mut buffer), 0) };
    if written <= 1 {
        return String::new();
    }
    // written 含結尾的 0，所以真正的字數要減 1
    let length = (written - 1) as usize;
    String::from_utf16_lossy(&buffer[..length])
}

/// 其他作業系統：讀不到地區。
#[cfg(not(windows))]
pub fn home_region_code() -> String {
    String::new()
}
