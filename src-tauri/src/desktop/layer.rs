//! 檔案：desktop/layer.rs
//! 用途：小工具「換層」這一半（S-001 顯示桌面時浮上來、單一執行個體閃一下都會用到）：
//!       - 判斷規則（純函式，有單元測試）：前景視窗是哪一種（classify_class_name）、
//!         小工具要浮上來／放回最下層／不動（decide）；
//!       - 記住「現在是不是因為顯示桌面浮在最上層」（is_raised／set_raised）；
//!       - 真正換層：浮到最上層或放回桌面最下層（move_to_layer），換層後補回工具視窗樣式（不上工作列與 Alt+Tab），
//!         玻璃主題再把材質標成作用中、維持毛玻璃（switch_layer）。
//!       「什麼時候該換層」的偵測（Windows 事件通知、0.2 秒後再看一次）在 desktop/show_desktop.rs。
//! 誰會用到：desktop/show_desktop.rs（判斷規則與 RAISED 狀態）；
//!           widget_window.rs 經由 desktop.rs（顯示桌面時換層、單一執行個體閃爍結束時放回原本那一層）。

use std::sync::atomic::{AtomicBool, Ordering};
use tauri::WebviewWindow;

/// 前景視窗是哪一種（決定小工具要不要換層）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForegroundKind {
    /// 小工具自己（使用者點了浮上來的小工具）
    Widget,
    /// 桌面（Windows 的桌面視窗類別 WorkerW、Progman）
    Desktop,
    /// 工作列（主螢幕與其他螢幕的工作列）
    Taskbar,
    /// 其他視窗（其他程式，也包含本程式的設定視窗）
    Other,
}

/// 小工具接下來要怎麼做。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerAction {
    /// 暫時浮到最上層
    Raise,
    /// 放回桌面最下層
    Lower,
    /// 維持現況
    Keep,
}

/// 換層時要呼叫的函式：參數 true＝浮上來、false＝放回最下層。
pub type LayerCallback = Box<dyn Fn(bool) + Send + Sync>;

/// 目前小工具是不是因為「顯示桌面」浮在最上層。
static RAISED: AtomicBool = AtomicBool::new(false);

/// 目前小工具是不是因為「顯示桌面」浮在最上層（單一執行個體閃爍結束時用來決定放回哪一層）。
pub fn is_raised() -> bool {
    RAISED.load(Ordering::Acquire)
}

/// 記下小工具現在是不是因為「顯示桌面」浮在最上層（由偵測端在決定換層時呼叫）。
pub fn set_raised(raised: bool) {
    RAISED.store(raised, Ordering::Release);
}

/// 由視窗類別名稱判斷前景視窗是哪一種（是不是小工具自己由呼叫端先判斷）。
pub fn classify_class_name(class_name: &str) -> ForegroundKind {
    match class_name {
        // 桌面：Windows 10／11 的桌面圖示放在 WorkerW 或 Progman 裡
        "WorkerW" | "Progman" => ForegroundKind::Desktop,
        // 工作列：主螢幕是 Shell_TrayWnd，其他螢幕是 Shell_SecondaryTrayWnd
        "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" => ForegroundKind::Taskbar,
        _ => ForegroundKind::Other,
    }
}

/// 決定小工具要浮上來、放回最下層，還是不動。
/// 輸入：raised＝目前是否浮在上面；kind＝新的前景視窗種類；
///       desktop_covers_widget＝桌面圖層是否蓋在小工具上面（只有「前景是桌面、還沒浮上來」時才有意義）。
pub fn decide(raised: bool, kind: ForegroundKind, desktop_covers_widget: bool) -> LayerAction {
    match kind {
        // 點了小工具自己、或點了工作列：維持現況
        ForegroundKind::Widget | ForegroundKind::Taskbar => LayerAction::Keep,
        ForegroundKind::Desktop => {
            if raised {
                // 已經浮上來了（例如顯示桌面時又點一下桌面）：維持
                return LayerAction::Keep;
            }
            if desktop_covers_widget {
                // Win+D：桌面被提到上面、蓋住小工具 → 浮上來
                LayerAction::Raise
            } else {
                // 只是點一下桌面空白處：桌面仍在最下面，小工具本來就看得到 → 不動
                LayerAction::Keep
            }
        }
        ForegroundKind::Other => {
            if raised {
                // 切回其他視窗：放回最下層，不蓋住別的視窗
                LayerAction::Lower
            } else {
                LayerAction::Keep
            }
        }
    }
}

/// 把小工具放到最上層（on_top = true）或桌面最下層（false）。
/// 先解除另一個狀態再設定：「最下層」和「最上層」同時存在時，Windows 仍會把它壓到最下面。
/// 做完要再補回工具視窗樣式（改這兩個旗標會讓 Tauri 洗掉它），switch_layer 會一起做。
pub fn move_to_layer(window: &WebviewWindow, on_top: bool) {
    if on_top {
        let _ = window.set_always_on_bottom(false);
        let _ = window.set_always_on_top(true);
    } else {
        let _ = window.set_always_on_top(false);
        let _ = window.set_always_on_bottom(true);
    }
}

/// 換層的完整動作：換到最上層或最下層 → 補回工具視窗樣式（不上 Alt+Tab）→
/// 玻璃主題（keep_glass_active = true）再把材質標成作用中，維持毛玻璃、不變成一片灰。
/// 在背景執行緒上呼叫；Tauri 會把視窗操作排到主執行緒依序處理。
pub fn switch_layer(window: &WebviewWindow, on_top: bool, keep_glass_active: bool) {
    move_to_layer(window, on_top);
    super::keep_off_taskbar_and_alt_tab(window);
    if keep_glass_active {
        super::keep_acrylic_active(window);
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_class_name, decide, ForegroundKind, LayerAction};

    #[test]
    fn recognizes_desktop_and_taskbar_windows() {
        assert_eq!(classify_class_name("WorkerW"), ForegroundKind::Desktop);
        assert_eq!(classify_class_name("Progman"), ForegroundKind::Desktop);
        assert_eq!(
            classify_class_name("Shell_TrayWnd"),
            ForegroundKind::Taskbar
        );
        assert_eq!(
            classify_class_name("Shell_SecondaryTrayWnd"),
            ForegroundKind::Taskbar
        );
        assert_eq!(
            classify_class_name("Chrome_WidgetWin_1"),
            ForegroundKind::Other
        );
        assert_eq!(classify_class_name(""), ForegroundKind::Other);
    }

    #[test]
    fn raises_only_when_desktop_covers_the_widget() {
        // Win+D：桌面變成前景而且蓋住小工具
        assert_eq!(
            decide(false, ForegroundKind::Desktop, true),
            LayerAction::Raise
        );
        // 只是點一下桌面空白處：桌面沒有蓋住小工具
        assert_eq!(
            decide(false, ForegroundKind::Desktop, false),
            LayerAction::Keep
        );
        // 已經浮上來，又點一下桌面：維持
        assert_eq!(
            decide(true, ForegroundKind::Desktop, false),
            LayerAction::Keep
        );
    }

    #[test]
    fn lowers_when_switching_to_another_window() {
        assert_eq!(
            decide(true, ForegroundKind::Other, false),
            LayerAction::Lower
        );
        // 本來就在最下層：切換其他視窗不用動
        assert_eq!(
            decide(false, ForegroundKind::Other, false),
            LayerAction::Keep
        );
    }

    #[test]
    fn keeps_layer_for_widget_and_taskbar() {
        assert_eq!(
            decide(true, ForegroundKind::Widget, false),
            LayerAction::Keep
        );
        assert_eq!(
            decide(true, ForegroundKind::Taskbar, false),
            LayerAction::Keep
        );
        assert_eq!(
            decide(false, ForegroundKind::Taskbar, false),
            LayerAction::Keep
        );
    }
}
