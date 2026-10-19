//! 檔案：scheduler/mod.rs
//! 用途：背景排程與 Refresh：每 30 秒檢查一次是否該查、三家同時查、查詢中鎖（不重複查）、
//!       合併結果、寫 cache.json／state.json，並通知畫面（usage-updated、refresh-state 事件）。
//!       對應 FR-1～FR-3 與 40「資料流」。
//!       三家同時查詢的執行細節在 jobs.rs；state.json 相關的操作（視窗位置、保險絲、看過的額度）在 runtime.rs。
//! 誰會用到：lib.rs（啟動時建立並開始排程）、commands.rs、settings_commands.rs、widget_window.rs。

mod jobs;
mod rules;
mod runtime;

pub use runtime::RuntimeSummary;

#[cfg(test)]
mod tests;

use crate::cache::{self, UsageSnapshot};
use crate::runtime_state::{self, RuntimeState};
use crate::settings::SettingsStore;
use jobs::run_jobs_in_parallel;
use rules::{
    build_jobs, interval_due, latest_attempt, merge_outcomes, next_unhandled_reset,
    prune_handled_resets, update_runtime_state,
};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// 背景檢查「該不該查」的間隔。FR-2 規定為 30 秒（電腦睡眠醒來後 30 秒內會補查），不是使用者設定。
const CHECK_INTERVAL: Duration = Duration::from_secs(30);

/// refresh-state 事件的內容：{ running: true/false }
#[derive(Debug, Clone, Serialize)]
struct RefreshState {
    running: bool,
}

/// 排程器。Clone 只是複製指標，大家共用同一份內部狀態。
#[derive(Clone)]
pub struct Scheduler {
    inner: Arc<SchedulerInner>,
}

struct SchedulerInner {
    /// 目前的用量資料（畫面讀的就是這一份）
    snapshot: Mutex<UsageSnapshot>,
    /// state.json 的內容（保險絲、視窗位置、看過的額度）
    runtime_state: Mutex<RuntimeState>,
    cache_path: PathBuf,
    state_path: PathBuf,
    /// 查詢中鎖：true 代表有一輪查詢正在進行
    running: AtomicBool,
    /// 上次「嘗試」查詢完成的時間（epoch 秒）
    last_attempt_at: Mutex<Option<i64>>,
    /// 已經為它補查過的重置時間（同一個重置時間只補查一次）
    handled_resets: Mutex<Vec<i64>>,
    /// 叫醒背景排程用（設定改了更新間隔時，不必等滿 30 秒才用新間隔判斷）
    wake_sender: Mutex<Option<Sender<()>>>,
}

/// 取得鎖；萬一鎖壞掉（其他執行緒 panic）仍取出內容，避免整個程式停擺。
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// 現在時間（epoch 秒）。
fn now_seconds() -> i64 {
    chrono::Utc::now().timestamp()
}

impl Scheduler {
    /// 用啟動時讀到的快取與狀態建立排程器；「上次嘗試」從快取接續計時。
    pub fn new(
        snapshot: UsageSnapshot,
        runtime_state: RuntimeState,
        cache_path: PathBuf,
        state_path: PathBuf,
    ) -> Self {
        let last_attempt_at = latest_attempt(&snapshot);
        Self {
            inner: Arc::new(SchedulerInner {
                snapshot: Mutex::new(snapshot),
                runtime_state: Mutex::new(runtime_state),
                cache_path,
                state_path,
                running: AtomicBool::new(false),
                last_attempt_at: Mutex::new(last_attempt_at),
                handled_resets: Mutex::new(Vec::new()),
                wake_sender: Mutex::new(None),
            }),
        }
    }

    /// 取得目前用量資料的複本（get_snapshot 指令）。
    pub fn snapshot(&self) -> UsageSnapshot {
        lock(&self.inner.snapshot).clone()
    }

    /// 開始背景排程：啟動時先檢查一次，之後每 30 秒檢查一次（被 wake() 叫醒時提早檢查）。
    pub fn start(&self, app_handle: AppHandle) {
        let (sender, receiver) = mpsc::channel::<()>();
        *lock(&self.inner.wake_sender) = Some(sender);

        let scheduler = self.clone();
        thread::spawn(move || loop {
            if let Some(reason) = scheduler.due_reason(&app_handle) {
                scheduler.request_refresh(app_handle.clone(), reason);
            }
            // 等 30 秒；期間被叫醒就馬上進下一輪檢查
            match receiver.recv_timeout(CHECK_INTERVAL) {
                Ok(()) | Err(RecvTimeoutError::Timeout) => {}
                // 叫醒用的通道不見了（理論上不會發生）：改成單純等待，避免空轉
                Err(RecvTimeoutError::Disconnected) => thread::sleep(CHECK_INTERVAL),
            }
        });
    }

    /// 叫醒背景排程，立刻用最新設定判斷該不該查（例如更新間隔剛改短）。
    pub fn wake(&self) {
        if let Some(sender) = lock(&self.inner.wake_sender).as_ref() {
            let _ = sender.send(());
        }
    }

    /// 要求查詢一輪。已經在查時不另起查詢，呼叫端等這一輪的事件即可（FR-3）。
    /// 回傳 true 代表真的開始了新的一輪。
    pub fn request_refresh(&self, app_handle: AppHandle, reason: &str) -> bool {
        let was_idle = self
            .inner
            .running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok();
        if !was_idle {
            log::info!("收到查詢要求（{reason}），但查詢進行中，沿用這一輪的結果");
            return false;
        }

        log::info!("開始查詢（{reason}）");
        let scheduler = self.clone();
        thread::spawn(move || scheduler.run_refresh(app_handle));
        true
    }

    /// 判斷現在該不該查；該查就回傳原因（寫 log 用），不該查回傳 None。
    fn due_reason(&self, app_handle: &AppHandle) -> Option<&'static str> {
        let settings = app_handle.state::<SettingsStore>().current();
        let now = now_seconds();

        let last_attempt = *lock(&self.inner.last_attempt_at);
        if interval_due(last_attempt, settings.general.refresh_interval_minutes, now) {
            return Some("已達更新間隔");
        }

        // 任一額度的重置時間已過：補查一次（同一個重置時間只觸發一次）
        let snapshot = self.snapshot();
        let mut handled = lock(&self.inner.handled_resets);
        prune_handled_resets(&mut handled, &snapshot);
        if let Some(reset_at) = next_unhandled_reset(&snapshot, &settings, &handled, now) {
            handled.push(reset_at);
            return Some("額度重置時間已過");
        }
        None
    }

    /// 真正執行一輪查詢（在背景執行緒）。
    fn run_refresh(&self, app_handle: AppHandle) {
        // 不論這個函式從哪裡結束，guard 都會解除查詢中鎖並通知畫面停止轉圈
        let _guard = RunningGuard {
            scheduler: self.clone(),
            app_handle: app_handle.clone(),
        };
        let _ = app_handle.emit("refresh-state", RefreshState { running: true });

        let settings = app_handle.state::<SettingsStore>().current();
        let fuse_tripped = lock(&self.inner.runtime_state).antigravity_fuse.tripped;
        let timeout_seconds = u64::try_from(settings.general.query_timeout_seconds).unwrap_or(45);
        let timeout = Duration::from_secs(timeout_seconds);

        // 啟用中的各家同時查詢（各開一條執行緒）
        let jobs = build_jobs(&settings, fuse_tripped);
        let outcomes = run_jobs_in_parallel(jobs, timeout);
        let finished_at = now_seconds();

        // 合併結果：成功覆蓋、失敗保留舊數字
        let mut snapshot = self.snapshot();
        merge_outcomes(&mut snapshot, &outcomes, &settings, finished_at);
        snapshot.updated_at = Some(finished_at);

        // state.json：保險絲
        {
            let mut state = lock(&self.inner.runtime_state);
            let newly_tripped = update_runtime_state(&mut state, &outcomes, finished_at);
            if newly_tripped {
                log::warn!(
                    "Antigravity 保險絲跳脫（num_turns={:?}），已停止查詢 Antigravity；請到設定畫面確認後手動恢復",
                    state.antigravity_fuse.num_turns
                );
            }
            if let Err(error) = runtime_state::save(&self.inner.state_path, &state) {
                log::error!("寫入 state.json 失敗：{error}");
            }
        }

        // cache.json：先寫暫存檔再改名
        if let Err(error) = cache::write_atomic(&self.inner.cache_path, &snapshot) {
            log::error!("寫入 cache.json 失敗：{error}");
        }

        *lock(&self.inner.snapshot) = snapshot.clone();
        // 下一次自動更新的計時，從這一輪結束的這一刻重新起算（FR-3）
        *lock(&self.inner.last_attempt_at) = Some(finished_at);

        log::info!("本輪查詢完成，已寫入快取");
        let _ = app_handle.emit("usage-updated", snapshot);
    }
}

/// 查詢結束（或中途出錯）時自動解除查詢中鎖，並發出 refresh-state { running: false }。
struct RunningGuard {
    scheduler: Scheduler,
    app_handle: AppHandle,
}

impl Drop for RunningGuard {
    fn drop(&mut self) {
        self.scheduler.inner.running.store(false, Ordering::Release);
        let _ = self
            .app_handle
            .emit("refresh-state", RefreshState { running: false });
    }
}
