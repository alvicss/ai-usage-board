//! 檔案：providers/locate.rs
//! 用途：依設定路徑、PATH 與 Antigravity 預設安裝位置安全尋找官方 CLI（45「CLI 位置偵測」）。
//! 誰會用到：三家 provider 在啟動查詢行程之前；設定畫面的「自動偵測結果」（detect_cli 指令）。

use super::{ProviderError, ProviderId, ProviderResult, StatusCode};
use std::env;
use std::path::{Path, PathBuf};

/// 找出這一家要用的 CLI：設定有填位置就用設定的（必須存在且是 .exe／.cmd），空白就自動偵測。
pub fn locate_cli(provider: ProviderId, configured_path: &str) -> ProviderResult<PathBuf> {
    if !configured_path.is_empty() {
        return validate_executable_path(Path::new(configured_path));
    }

    match auto_detect_cli(provider) {
        Some(path) => Ok(path),
        None => Err(ProviderError::new(
            StatusCode::NotInstalled,
            format!("找不到 {} CLI", command_name(provider)),
        )),
    }
}

/// 自動偵測：依 PATH＋PATHEXT 找（能找到 .cmd），Antigravity 另外找預設安裝位置；找不到回傳 None。
/// 只看檔案位置，不執行 CLI。
pub fn auto_detect_cli(provider: ProviderId) -> Option<PathBuf> {
    if let Ok(path) = which::which(command_name(provider)) {
        if validate_executable_path(&path).is_ok() {
            return Some(path);
        }
    }

    if provider == ProviderId::Antigravity {
        if let Some(path) = antigravity_default_path() {
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

/// 各家 CLI 的指令名稱。
fn command_name(provider: ProviderId) -> &'static str {
    match provider {
        ProviderId::Claude => "claude",
        ProviderId::Codex => "codex",
        ProviderId::Antigravity => "agy",
    }
}

fn validate_executable_path(path: &Path) -> ProviderResult<PathBuf> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    let allowed_extension =
        extension.eq_ignore_ascii_case("exe") || extension.eq_ignore_ascii_case("cmd");

    if path.is_file() && allowed_extension {
        return Ok(path.to_path_buf());
    }

    Err(ProviderError::new(
        StatusCode::NotInstalled,
        "CLI 路徑不存在，或不是 .exe／.cmd",
    ))
}

fn antigravity_default_path() -> Option<PathBuf> {
    let local_app_data = env::var_os("LOCALAPPDATA")?;
    Some(
        PathBuf::from(local_app_data)
            .join("agy")
            .join("bin")
            .join("agy.exe"),
    )
}

#[cfg(test)]
mod tests {
    use super::validate_executable_path;
    use crate::providers::StatusCode;
    use std::path::Path;

    #[test]
    fn rejects_missing_configured_path() {
        let error = validate_executable_path(Path::new("Z:\\missing\\claude.exe"))
            .expect_err("不存在的 CLI 路徑必須被拒絕");
        assert_eq!(error.status, StatusCode::NotInstalled);
    }
}
