//! 檔案：main.rs
//! 用途：Windows 桌面程式的最小進入點，並避免額外開啟黑色主控台視窗。
//! 誰會用到：作業系統啟動 AI Usage Board 執行檔時呼叫。

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() {
    ai_usage_board_lib::run();
}
