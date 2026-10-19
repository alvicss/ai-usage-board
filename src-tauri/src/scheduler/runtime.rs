//! 檔案：scheduler/runtime.rs
//! 用途：排程器裡和 state.json（41「狀態檔」）有關的操作：
//!       - 兩種顯示方式各自的視窗位置大小（M-005）
//!       - Antigravity 保險絲的查詢與手動恢復（45「Antigravity 保險絲」）
//!       state.json 的內容由排程器統一保管（查詢完成時也會寫入），所以這些操作放在排程器上，避免兩邊同時寫檔。
//! 誰會用到：widget_window.rs（視窗位置）、settings_commands.rs（保險絲）。

use super::{lock, Scheduler};
use crate::runtime_state::{self, AntigravityFuse, RuntimeState, WindowBounds};
use serde::Serialize;

/// 給設定畫面的狀態摘要（get_runtime_state 指令的回傳內容；不含視窗位置）。
#[derive(Debug, Clone, Serialize)]
pub struct RuntimeSummary {
    pub antigravity_fuse: AntigravityFuse,
}

impl Scheduler {
    /// 取得某個顯示方式（carousel／overview）記住的位置大小；沒記過就是 None。
    pub fn window_bounds(&self, mode: &str) -> Option<WindowBounds> {
        let state = lock(&self.inner.runtime_state);
        if mode == "overview" {
            state.window.overview
        } else {
            state.window.carousel
        }
    }

    /// 記住某個顯示方式的位置大小，以及它在哪個螢幕（螢幕名稱只供 log 與除錯參考）。
    pub fn save_window_bounds(
        &self,
        mode: &str,
        bounds: WindowBounds,
        monitor_name: &str,
    ) -> Result<(), String> {
        let mut state = lock(&self.inner.runtime_state);
        match mode {
            "carousel" => state.window.carousel = Some(bounds),
            "overview" => state.window.overview = Some(bounds),
            _ => return Err("未知的顯示模式".to_string()),
        }
        state.window.monitor = monitor_name.to_string();
        self.write_state(&state)
    }

    /// 設定畫面要顯示的狀態：保險絲。
    pub fn runtime_summary(&self) -> RuntimeSummary {
        let state = lock(&self.inner.runtime_state);
        RuntimeSummary {
            antigravity_fuse: state.antigravity_fuse.clone(),
        }
    }

    /// 使用者在設定畫面按「我已確認，恢復查詢」：重設 Antigravity 保險絲。
    pub fn reset_antigravity_fuse(&self) -> Result<(), String> {
        let mut state = lock(&self.inner.runtime_state);
        state.antigravity_fuse = AntigravityFuse::default();
        log::info!("使用者已確認，Antigravity 保險絲重設，恢復查詢");
        self.write_state(&state)
    }

    /// 安全寫入 state.json（先寫暫存檔再改名）。
    fn write_state(&self, state: &RuntimeState) -> Result<(), String> {
        runtime_state::save(&self.inner.state_path, state).map_err(|error| error.to_string())
    }
}
