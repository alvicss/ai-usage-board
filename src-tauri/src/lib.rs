//! 檔案：lib.rs
//! 用途：組裝整個 Tauri 程式：註冊外掛與指令，啟動時依序準備資料夾 → 讀設定 → 開 log →
//!       開機自動啟動重新登記 → 讀快取與狀態 → 建立排程器 → 擺好小工具視窗 → 開始背景排程 → 開始監聽 Win+D →
//!       建立系統匣圖示；
//!       並監聽小工具視窗的事件（失去焦點、Windows 切換亮暗時維持玻璃材質）。
//! 誰會用到：main.rs（程式進入點）。

mod about;
mod atomic_file;
mod autostart;
mod cache;
mod commands;
mod desktop;
mod logging;
mod paths;
mod providers;
mod runtime_state;
mod scheduler;
mod settings;
mod settings_commands;
mod settings_window;
mod system_info;
mod tray;
mod widget_window;
mod window_placement;

use tauri::{Manager, WindowEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // 只允許一個執行個體（FR-14）：必須最先註冊；再開一次時不開第二個，改讓現有小工具閃一下
        .plugin(tauri_plugin_single_instance::init(|app, _arguments, _working_directory| {
            widget_window::flash(app, "偵測到重複開啟");
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::refresh_now,
            commands::set_display_mode,
            commands::save_window_bounds,
            commands::open_settings,
            commands::close_settings,
            commands::quit_app,
            settings_commands::get_settings,
            settings_commands::check_settings,
            settings_commands::save_settings,
            settings_commands::detect_cli,
            settings_commands::get_runtime_state,
            settings_commands::reset_antigravity_fuse,
            about::get_app_info,
            about::open_homepage,
            about::open_repository,
            system_info::get_system_info
        ])
        // 小工具失去焦點、或 Windows 切換淺色／深色之後，玻璃材質要重新標成「作用中」才不會變灰（desktop.rs）
        .on_window_event(|window, event| {
            if window.label() != widget_window::WIDGET_LABEL {
                return;
            }
            match event {
                WindowEvent::Focused(false) | WindowEvent::ThemeChanged(_) => {
                    widget_window::keep_acrylic_active(window.app_handle());
                }
                _ => {}
            }
        })
        .setup(|app| {
            // 1. 準備 %APPDATA%\AI Usage Board\ 與 logs 子資料夾
            let app_directories = paths::AppDirectories::prepare(app.handle())?;

            // 2. 讀設定（第一次啟動會建立 settings.toml；壞檔時用預設值並留下警告）
            let settings_store = settings::SettingsStore::load(&app_directories.settings)?;
            let settings = settings_store.current();

            // 3. 開 log，並把讀設定時的警告寫進去
            logging::initialize(&app_directories.logs, settings.general.log_retention_days)?;
            // 萬一發生未預期的錯誤（panic），先把原因寫進 log 再結束，小工具突然消失時才查得到原因
            logging::install_panic_hook();
            log::info!("AI Usage Board 啟動（版本 {}）", env!("CARGO_PKG_VERSION"));
            for warning in settings_store.load_warnings() {
                log::warn!("設定警告：{}", warning.log_text());
            }

            // 4. 開機自動啟動：設定是開的就重新登記目前 exe 位置（FR-10，免安裝版搬家後仍有效）
            if settings.general.autostart {
                autostart::apply(app.handle(), true);
            }

            // 5. 讀快取（畫面立刻有上次的數字）與 state.json，建立排程器
            let snapshot = cache::load(&app_directories.cache, &settings);
            let runtime_state = runtime_state::load(&app_directories.state);
            let scheduler = scheduler::Scheduler::new(
                snapshot,
                runtime_state,
                app_directories.cache.clone(),
                app_directories.state.clone(),
            );

            // 6. 擺好小工具：恢復位置大小（第一次在主螢幕右上角）、套用亮暗與玻璃材質、
            //    釘在桌面最下層、不上工作列與 Alt+Tab
            widget_window::prepare_on_startup(app.handle(), &scheduler, &settings)?;

            // 7. 交給 Tauri 保管，並開始背景檢查（需要的話馬上查一次）
            app.manage(settings_store);
            app.manage(scheduler.clone());
            scheduler.start(app.handle().clone());

            // 8. 開始監聽 Win+D（顯示桌面）：偵測到時小工具暫時浮上來（S-001）
            widget_window::start_show_desktop_watcher(app.handle());

            // 9. Windows 右下角通知區的系統匣圖示（M-010）：右鍵選單可以結束程式，左鍵讓小工具閃一下
            tray::create(app.handle(), &settings.general.language);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("AI Usage Board 啟動失敗");
}
