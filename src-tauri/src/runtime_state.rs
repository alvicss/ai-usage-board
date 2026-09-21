//! 檔案：runtime_state.rs
//! 用途：讀寫 state.json（41「狀態檔」）：Antigravity 保險絲、兩種模式各自的視窗位置大小、看過的額度 ID。
//!       保險絲狀態重開程式後仍保留，使用者在設定畫面確認後才重設。
//! 誰會用到：lib.rs（啟動時讀取）、scheduler（保管內容：保險絲、看過的額度、視窗位置；widget_window.rs 透過它恢復與記住視窗位置）。

use crate::atomic_file;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// state.json 的完整內容。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeState {
    pub schema_version: u32,
    #[serde(default)]
    pub window: WindowState,
    #[serde(default)]
    pub antigravity_fuse: AntigravityFuse,
    #[serde(default)]
    pub seen_quota_ids: Vec<String>,
}

/// 視窗位置與大小：輪播、並排兩種模式各自記住（M-005）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WindowState {
    #[serde(default)]
    pub monitor: String,
    #[serde(default)]
    pub carousel: Option<WindowBounds>,
    #[serde(default)]
    pub overview: Option<WindowBounds>,
}

/// 一組位置與大小（實體像素）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Antigravity 保險絲狀態（45「Antigravity 保險絲」）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AntigravityFuse {
    pub tripped: bool,
    pub tripped_at: Option<i64>,
    pub num_turns: Option<f64>,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            schema_version: 1,
            window: WindowState::default(),
            antigravity_fuse: AntigravityFuse::default(),
            seen_quota_ids: Vec::new(),
        }
    }
}

/// 讀取 state.json；不存在或壞掉時用預設狀態（保險絲未跳脫）。
pub fn load(path: &Path) -> RuntimeState {
    let Ok(text) = fs::read_to_string(path) else {
        return RuntimeState::default();
    };
    match serde_json::from_str::<RuntimeState>(&text) {
        Ok(state) if state.schema_version == 1 => state,
        _ => {
            log::warn!("state.json 無法解析或版本不相容，本次使用預設狀態");
            RuntimeState::default()
        }
    }
}

/// 安全寫入 state.json（先寫暫存檔再改名）。
pub fn save(path: &Path, state: &RuntimeState) -> Result<(), std::io::Error> {
    let bytes = serde_json::to_vec_pretty(state).map_err(std::io::Error::other)?;
    atomic_file::write_atomic(path, &bytes)
}

#[cfg(test)]
mod tests {
    use super::{load, save, RuntimeState};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn fuse_state_survives_round_trip() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("系統時間應有效")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("ai-usage-state-{unique}"));
        std::fs::create_dir_all(&directory).expect("應建立測試資料夾");
        let path = directory.join("state.json");
        let mut state = RuntimeState::default();
        state.antigravity_fuse.tripped = true;
        state.antigravity_fuse.num_turns = Some(1.0);
        save(&path, &state).expect("應寫入狀態");
        save(&path, &state).expect("第二次寫入應蓋過舊檔");
        let loaded = load(&path);
        assert!(loaded.antigravity_fuse.tripped);
        assert_eq!(loaded.antigravity_fuse.num_turns, Some(1.0));
        std::fs::remove_dir_all(directory).expect("應清除測試資料");
    }
}
