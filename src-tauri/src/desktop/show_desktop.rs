//! 檔案：desktop/show_desktop.rs
//! 用途：S-001「按 Win+D（顯示桌面）時，小工具暫時浮上來」的**偵測**這一半（類似 Rainmeter）：
//!       - 用 Windows 的事件通知（SetWinEventHook）得知「前景視窗換了」（EVENT_SYSTEM_FOREGROUND）與
//!         「頂層視窗的前後順序變了」（桌面視窗的 EVENT_OBJECT_REORDER），不用一直輪詢；
//!         為什麼兩種都聽：Windows 11 按 Win+D 時，「前景換成桌面」的通知可能要等到下一次切換才送來，
//!         只聽前景通知會偵測不到；Win+D 把桌面提上來時一定會有「前後順序變了」的通知。
//!       - 每次收到通知，都重新看「目前真正的前景視窗」與「桌面有沒有蓋住小工具」，另外 0.2 秒後再看一次（通知可能比實際變化早到）；
//!       - 看完交給 desktop/layer.rs 的判斷規則（decide）決定浮上來、放回最下層或不動，要換層時呼叫 on_change。
//!       通知在專用的背景執行緒接收（這條執行緒自己跑訊息迴圈），不在 Tauri 的主執行緒上處理，
//!       避免在主執行緒處理視窗訊息的途中又去改視窗（22 踩過的坑）。真正換層在 desktop/layer.rs（由 widget_window.rs 呼叫）。
//! 誰會用到：desktop.rs 對外提供 start_show_desktop_watcher。

#[cfg(windows)]
use super::layer::LayerCallback;

// ---------------------------------------------------------------------------
// Windows 實作：事件通知、判斷桌面是否蓋住小工具
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod windows_impl {
    use super::super::layer::{
        classify_class_name, decide, is_raised, set_raised, ForegroundKind, LayerAction,
        LayerCallback,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::OnceLock;
    use std::thread;
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, FindWindowExW, GetAncestor, GetClassNameW, GetDesktopWindow,
        GetForegroundWindow, GetMessageW, GetWindow, KillTimer, SetTimer, TranslateMessage,
        EVENT_OBJECT_REORDER, EVENT_SYSTEM_FOREGROUND, GA_ROOT, GW_HWNDPREV, MSG,
        WINEVENT_OUTOFCONTEXT,
    };

    /// 收到通知後，再隔多久重新看一次（毫秒）：通知可能比「前景真的換成桌面」早到，
    /// 0.2 秒後補看一次就不會漏掉，使用者也感覺不到延遲。
    const RECHECK_DELAY_MS: u32 = 200;
    /// 往上數頂層視窗的安全上限（一般電腦只有幾百個，避免視窗清單異常時一直數下去）。
    const MAX_WINDOWS_TO_CHECK: u32 = 5000;

    /// 事件回呼函式沒辦法帶自訂參數，所以把需要的資料放在全域（只設定一次）。
    struct Watcher {
        /// 小工具視窗代碼（存成數字，才能在執行緒之間傳遞）
        widget_hwnd: isize,
        /// 換層時要呼叫的函式
        on_change: LayerCallback,
    }

    static WATCHER: OnceLock<Watcher> = OnceLock::new();
    /// 「0.2 秒後再看一次」的計時器代碼；0 代表目前沒有排定。
    static RECHECK_TIMER: AtomicUsize = AtomicUsize::new(0);

    /// 開始監聽：開一條背景執行緒，在裡面登記事件通知並跑訊息迴圈（整個程式執行期間都在）。
    pub fn start(widget_hwnd: isize, on_change: LayerCallback) {
        let watcher = Watcher {
            widget_hwnd,
            on_change,
        };
        if WATCHER.set(watcher).is_err() {
            log::warn!("「顯示桌面」監聽已經啟動過，不重複啟動");
            return;
        }

        let spawn_result = thread::Builder::new()
            .name("show-desktop-watcher".to_string())
            .spawn(run_message_loop);
        if let Err(error) = spawn_result {
            log::warn!("無法啟動「顯示桌面」監聽，Win+D 時小工具不會浮上來：{error}");
        }
    }

    /// 登記一種事件通知；WINEVENT_OUTOFCONTEXT 代表通知由 Windows 排進本執行緒的訊息佇列，
    /// 在訊息迴圈裡呼叫 on_window_event，不需要把程式注入其他行程。
    fn register_hook(event: u32) -> HWINEVENTHOOK {
        // SAFETY：回呼函式是本檔的 on_window_event，整個程式執行期間都存在。
        unsafe {
            SetWinEventHook(
                event,
                event,
                None,
                Some(on_window_event),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            )
        }
    }

    /// 背景執行緒：登記兩種事件通知，然後一直處理訊息（通知與計時器都是透過訊息送進來的）。
    fn run_message_loop() {
        let foreground_hook = register_hook(EVENT_SYSTEM_FOREGROUND);
        let reorder_hook = register_hook(EVENT_OBJECT_REORDER);
        if foreground_hook.is_invalid() || reorder_hook.is_invalid() {
            log::warn!("無法監聽視窗切換，Win+D 時小工具不會浮上來");
            return;
        }
        log::info!("開始監聽「顯示桌面」（Win+D）");

        let mut message = MSG::default();
        loop {
            // SAFETY：message 是本函式的區域變數；只取本執行緒的訊息。
            let result = unsafe { GetMessageW(&mut message, None, 0, 0) };
            // 0＝收到結束訊息、-1＝發生錯誤：都停止監聽
            if result.0 <= 0 {
                break;
            }
            // SAFETY：message 剛由 GetMessageW 填好。
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }

        // SAFETY：兩個 hook 都是上面登記成功的代碼。
        unsafe {
            let _ = UnhookWinEvent(foreground_hook);
            let _ = UnhookWinEvent(reorder_hook);
        }
    }

    /// Windows 的事件通知（在監聽執行緒上執行）：前景視窗換了，或頂層視窗的前後順序變了。
    unsafe extern "system" fn on_window_event(
        _hook: HWINEVENTHOOK,
        event: u32,
        hwnd: HWND,
        _id_object: i32,
        _id_child: i32,
        _event_thread: u32,
        _event_time: u32,
    ) {
        // 「前後順序變了」只看整個桌面（頂層視窗）這一層；其他程式視窗內部的順序變化跟我們無關
        if event == EVENT_OBJECT_REORDER {
            // SAFETY：只取得桌面視窗代碼。
            let desktop_window = unsafe { GetDesktopWindow() };
            if hwnd != desktop_window {
                return;
            }
        }
        update_layer();
        schedule_recheck();
    }

    /// 0.2 秒後再看一次（已經排定的話重新計時）。
    fn schedule_recheck() {
        let previous = RECHECK_TIMER.swap(0, Ordering::AcqRel);
        if previous != 0 {
            // SAFETY：previous 是本執行緒之前用 SetTimer 建立的計時器。
            unsafe {
                let _ = KillTimer(None, previous);
            }
        }
        // SAFETY：不指定視窗的計時器，到時由本執行緒的訊息迴圈呼叫 on_recheck_timer。
        let timer = unsafe { SetTimer(None, 0, RECHECK_DELAY_MS, Some(on_recheck_timer)) };
        RECHECK_TIMER.store(timer, Ordering::Release);
    }

    /// 計時器到了：只看一次，然後把計時器關掉。
    unsafe extern "system" fn on_recheck_timer(
        _hwnd: HWND,
        _message: u32,
        timer: usize,
        _time: u32,
    ) {
        // SAFETY：timer 是本執行緒建立的計時器。
        unsafe {
            let _ = KillTimer(None, timer);
        }
        let _ = RECHECK_TIMER.compare_exchange(timer, 0, Ordering::AcqRel, Ordering::Acquire);
        update_layer();
    }

    /// 看目前真正的前景視窗與桌面位置，決定小工具要浮上來、放回最下層或不動。
    fn update_layer() {
        let Some(watcher) = WATCHER.get() else {
            return;
        };
        // SAFETY：只讀取目前的前景視窗；切換途中可能暫時沒有，就等下一次通知。
        let foreground = unsafe { GetForegroundWindow() };
        if foreground.is_invalid() {
            return;
        }

        let kind = foreground_kind(foreground, watcher.widget_hwnd);
        let raised = is_raised();

        // 只有「前景是桌面、還沒浮上來」時才需要檢查桌面有沒有蓋住小工具（要數視窗，其他情況省下來）
        let mut desktop_covers_widget = false;
        if kind == ForegroundKind::Desktop && !raised {
            desktop_covers_widget = desktop_is_above(watcher.widget_hwnd);
        }

        match decide(raised, kind, desktop_covers_widget) {
            LayerAction::Raise => {
                set_raised(true);
                log::info!("偵測到「顯示桌面」：小工具暫時浮上來");
                (watcher.on_change)(true);
            }
            LayerAction::Lower => {
                set_raised(false);
                log::info!("切回其他視窗：小工具放回桌面最下層");
                (watcher.on_change)(false);
            }
            LayerAction::Keep => {}
        }
    }

    /// 前景視窗是哪一種。先找到它所在的頂層視窗，再看是不是小工具、否則依類別名稱判斷。
    fn foreground_kind(hwnd: HWND, widget_hwnd: isize) -> ForegroundKind {
        // SAFETY：只讀取視窗關係；hwnd 若已經關掉，Windows 回傳空值，不會出錯。
        let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
        let top_level = if root.is_invalid() { hwnd } else { root };
        if top_level.0 as isize == widget_hwnd {
            return ForegroundKind::Widget;
        }
        classify_class_name(&class_name(top_level))
    }

    /// 視窗的類別名稱（例如 WorkerW）；讀不到時是空字串。
    fn class_name(hwnd: HWND) -> String {
        let mut buffer = [0u16; 128];
        // SAFETY：buffer 是本函式的區域陣列，長度由切片自己帶給 Windows。
        let length = unsafe { GetClassNameW(hwnd, &mut buffer) };
        if length <= 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buffer[..length as usize])
    }

    /// 從小工具往上一個一個看頂層視窗，只要有「放桌面圖示的視窗」在小工具上面，就代表桌面蓋住了小工具。
    /// 平常桌面在所有視窗的最下面（小工具在它上面）；Win+D 時 Windows 把桌面提到一般視窗的最上面。
    fn desktop_is_above(widget_hwnd: isize) -> bool {
        let mut current = HWND(widget_hwnd as *mut core::ffi::c_void);
        for _step in 0..MAX_WINDOWS_TO_CHECK {
            // SAFETY：只讀取 z-order 關係；到最上面時 Windows 回傳錯誤，迴圈結束。
            let above = match unsafe { GetWindow(current, GW_HWNDPREV) } {
                Ok(window) => window,
                Err(_) => return false,
            };
            if hosts_desktop_icons(above) {
                return true;
            }
            current = above;
        }
        false
    }

    /// 這個頂層視窗裡有沒有桌面圖示（子視窗 SHELLDLL_DefView）；有的就是桌面圖層（Progman 或 WorkerW）。
    fn hosts_desktop_icons(hwnd: HWND) -> bool {
        // SAFETY：只在 hwnd 的子視窗中找指定類別；找不到時回傳錯誤。
        let found =
            unsafe { FindWindowExW(Some(hwnd), None, w!("SHELLDLL_DefView"), PCWSTR::null()) };
        found.is_ok()
    }
}

/// 開始監聽「顯示桌面」（只有 Windows）。widget_hwnd 是小工具的視窗代碼；on_change 在換層時被呼叫（在監聽執行緒上）。
#[cfg(windows)]
pub fn start(widget_hwnd: isize, on_change: LayerCallback) {
    windows_impl::start(widget_hwnd, on_change);
}
