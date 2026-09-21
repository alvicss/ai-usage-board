//! 檔案：about.rs
//! 用途：設定視窗「關於」頁需要的指令（FR-15）：
//!       - get_app_info：程式名稱、兩碼版本（例如 v1.0，取自程式版本的主版號與次版號）、授權；
//!       - open_homepage：把作者網站交給 Windows 預設瀏覽器開啟；
//!       - open_repository：把本專案的 GitHub 頁面（原始碼、下載新版、問題回報）交給預設瀏覽器開啟。
//!       兩個網址都寫死在這裡的常數，畫面端不能指定任何網址（22 資安規則 2：程式本身不連網、不在程式內開網頁）。
//! 誰會用到：lib.rs 註冊指令；畫面的 services/backend.ts（getAppInfo、openHomepage、openRepository）。

use serde::Serialize;
use tauri::AppHandle;

/// 作者「半桶水的AI筆記」的網站（FR-15）。
const HOMEPAGE_URL: &str = "https://halfmemo.com/";
/// 本專案的 GitHub 頁面（FR-15；M-009 建立）。
const REPOSITORY_URL: &str = "https://github.com/alvicss/ai-usage-board";
/// 授權類型（LICENSE 檔）。
const LICENSE_NAME: &str = "MIT";

/// get_app_info 的回傳內容。
#[derive(Debug, Clone, Serialize)]
pub struct AppInfo {
    /// 程式名稱（tauri.conf.json 的 productName）
    pub name: String,
    /// 兩碼版本，例如 "v1.0"
    pub version: String,
    /// 授權類型
    pub license: String,
}

/// 把完整版本號（例如 "1.0.0"、"1.2.3-beta"）換成兩碼顯示（"v1.0"、"v1.2"）。
/// 格式看不懂時，原樣加上 v 顯示。
pub fn short_version(full_version: &str) -> String {
    let parts: Vec<&str> = full_version.split('.').collect();
    if parts.len() < 2 {
        return format!("v{full_version}");
    }
    let major = parts[0];
    // 次版號後面可能還接著 "-beta" 之類的標記，只取數字部分
    let mut minor = String::new();
    for character in parts[1].chars() {
        if !character.is_ascii_digit() {
            break;
        }
        minor.push(character);
    }
    if major.is_empty() || minor.is_empty() {
        return format!("v{full_version}");
    }
    format!("v{major}.{minor}")
}

/// 關於頁：取得程式名稱、兩碼版本與授權。
#[tauri::command]
pub fn get_app_info(app: AppHandle) -> AppInfo {
    let package = app.package_info();
    AppInfo {
        name: package.name.clone(),
        version: short_version(&package.version.to_string()),
        license: LICENSE_NAME.to_string(),
    }
}

/// 關於頁點作者：用 Windows 預設瀏覽器開作者網站（async：交給 Windows 開瀏覽器時不卡住畫面）。
#[tauri::command]
pub async fn open_homepage() -> Result<(), String> {
    open_in_browser(HOMEPAGE_URL, "作者網站")
}

/// 關於頁點 GitHub：用 Windows 預設瀏覽器開本專案的 GitHub 頁面。
#[tauri::command]
pub async fn open_repository() -> Result<(), String> {
    open_in_browser(REPOSITORY_URL, "GitHub 頁面")
}

/// 把固定網址交給 Windows 預設瀏覽器開啟；description 只用在 log。
fn open_in_browser(url: &str, description: &str) -> Result<(), String> {
    match tauri_plugin_opener::open_url(url, None::<&str>) {
        Ok(()) => {
            log::info!("已請預設瀏覽器開啟{description}");
            Ok(())
        }
        Err(error) => {
            log::warn!("無法開啟預設瀏覽器：{error}");
            Err(error.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::short_version;

    #[test]
    fn shows_major_and_minor_only() {
        assert_eq!(short_version("1.0.0"), "v1.0");
        assert_eq!(short_version("2.13.4"), "v2.13");
    }

    #[test]
    fn ignores_pre_release_marks() {
        assert_eq!(short_version("1.2-beta.1"), "v1.2");
    }

    #[test]
    fn keeps_unknown_formats_as_they_are() {
        assert_eq!(short_version("dev"), "vdev");
    }
}
