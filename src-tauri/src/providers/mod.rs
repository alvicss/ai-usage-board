//! 檔案：providers/mod.rs
//! 用途：定義三家 AI 服務共用的用量格式、狀態與解析工具（含額度的排列順序 order_quotas）。
//! 誰會用到：Claude、Codex、Antigravity 查詢模組，以及後續排程與快取模組。

mod antigravity;
mod claude;
mod codex;
mod locate;
mod process;

#[cfg(test)]
mod live_capture;

pub use locate::auto_detect_cli;
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProviderId {
    Claude,
    Codex,
    Antigravity,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StatusCode {
    Pending,
    Ok,
    Disabled,
    NotInstalled,
    NotLoggedIn,
    Offline,
    Outdated,
    Timeout,
    ParseError,
    /// CLI 正常回應，但這個帳號沒有任何訂閱額度資料（例如沒訂閱、改用 API 金鑰按量付費）
    NoQuota,
    FuseTripped,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Quota {
    pub id: String,
    pub label_key: String,
    pub label_param: Option<String>,
    pub used_percent: f64,
    pub resets_at: Option<i64>,
    pub window_minutes: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderUsage {
    pub id: ProviderId,
    pub name: String,
    pub enabled: bool,
    pub plan: Option<String>,
    pub status: StatusCode,
    pub status_detail: Option<String>,
    pub last_attempt_at: Option<i64>,
    pub last_success_at: Option<i64>,
    pub quotas: Vec<Quota>,
}

/// 一家查詢失敗的原因。
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderError {
    /// 狀態代碼（見 41「狀態代碼」）
    pub status: StatusCode,
    /// 給 log 的簡短說明，不含帳號資訊與 CLI 原始回應
    pub detail: String,
    /// 只有 Antigravity 保險絲因 num_turns > 0 跳脫時才有值，會記進 state.json
    pub num_turns: Option<f64>,
}

impl ProviderError {
    /// 建立一筆失敗原因。
    pub fn new(status: StatusCode, detail: impl Into<String>) -> Self {
        Self {
            status,
            detail: detail.into(),
            num_turns: None,
        }
    }

    /// 附上 Antigravity 回報的 num_turns（保險絲跳脫時使用）。
    pub fn with_num_turns(mut self, num_turns: f64) -> Self {
        self.num_turns = Some(num_turns);
        self
    }
}

pub type ProviderResult<T> = Result<T, ProviderError>;
/// 依服務 ID 執行唯一允許的固定查詢，讓排程器不需要知道各家協定細節。
pub(crate) fn query(
    provider_id: ProviderId,
    configured_path: &str,
    timeout: Duration,
) -> ProviderResult<ProviderUsage> {
    match provider_id {
        ProviderId::Claude => claude::query(configured_path, timeout),
        ProviderId::Codex => codex::query(configured_path, timeout),
        ProviderId::Antigravity => antigravity::query(configured_path, timeout),
    }
}

/// 額度的排列順序：三家服務都是「5 小時在前、每週在後」。
/// 1. 先依群組：保持 CLI 回報的群組順序（例如 Antigravity 先 Gemini、再 Claude+GPT；
///    Claude 的單一模型額度、Codex 的額外額度是後面的群組，所以排在主要額度之後）；
/// 2. 同一群組內依週期由短到長（5 小時 300 分 → 每週 10080 分），不知道週期的排最後；
///    週期相同時維持原本的順序。
///
/// 輸入：每個額度與它所屬群組的名稱（依 CLI 回報的順序）。輸出：排好的額度。
fn order_quotas(grouped: Vec<(String, Quota)>) -> Vec<Quota> {
    // 1. 記下每個群組第一次出現的順序
    let mut group_order: Vec<String> = Vec::new();
    for (group, _) in &grouped {
        if !group_order.contains(group) {
            group_order.push(group.clone());
        }
    }

    // 2. 一個群組一個群組取出來；群組內依週期排序（sort_by_key 是穩定排序，週期相同時不改變順序）
    let mut ordered = Vec::new();
    for group in &group_order {
        let mut members: Vec<Quota> = Vec::new();
        for (member_group, quota) in &grouped {
            if member_group == group {
                members.push(quota.clone());
            }
        }
        members.sort_by_key(|quota| quota.window_minutes.unwrap_or(u32::MAX));
        ordered.extend(members);
    }
    ordered
}

fn clamp_percent(value: f64) -> f64 {
    value.clamp(0.0, 100.0)
}

fn parse_iso_timestamp(value: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|date_time| date_time.timestamp())
}

fn quota_label(window_minutes: Option<u32>) -> (&'static str, Option<String>) {
    match window_minutes {
        Some(300) => ("quota.five_hour", None),
        Some(10_080) => ("quota.weekly", None),
        Some(minutes) if minutes % 1_440 == 0 => {
            ("quota.window_days", Some((minutes / 1_440).to_string()))
        }
        Some(minutes) if minutes % 60 == 0 => {
            ("quota.window_hours", Some((minutes / 60).to_string()))
        }
        Some(minutes) => ("quota.window_minutes", Some(minutes.to_string())),
        None => ("quota.unknown_window", None),
    }
}

fn slugify(value: &str) -> String {
    let mut result = String::new();
    let mut previous_was_separator = false;

    for character in value.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            result.push(character);
            previous_was_separator = false;
        } else if !result.is_empty() && !previous_was_separator {
            result.push('-');
            previous_was_separator = true;
        }
    }

    while result.ends_with('-') {
        result.pop();
    }

    if result.is_empty() {
        "unknown".to_string()
    } else {
        result
    }
}

fn title_case_ascii(value: &str) -> String {
    let mut characters = value.chars();
    let Some(first_character) = characters.next() else {
        return String::new();
    };

    let mut result = first_character.to_uppercase().collect::<String>();
    result.push_str(characters.as_str());
    result
}

fn classify_process_failure(
    provider_name: &str,
    exited_successfully: bool,
    stderr: &str,
    stdout: &str,
) -> ProviderError {
    let combined = format!("{stderr}\n{stdout}").to_ascii_lowercase();

    let status = if combined.contains("not logged in")
        || combined.contains("login required")
        || combined.contains("authentication required")
        || combined.contains("unauthorized")
    {
        StatusCode::NotLoggedIn
    } else if combined.contains("offline")
        || combined.contains("network")
        || combined.contains("connection")
        || combined.contains("dns")
    {
        StatusCode::Offline
    } else if exited_successfully {
        StatusCode::ParseError
    } else {
        StatusCode::Error
    };

    ProviderError::new(
        status,
        format!("{provider_name} CLI 未回傳可解析的用量資料"),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        clamp_percent, classify_process_failure, order_quotas, quota_label, slugify,
        title_case_ascii, Quota, StatusCode,
    };

    fn quota(id: &str, window_minutes: Option<u32>) -> Quota {
        Quota {
            id: id.to_string(),
            label_key: "quota.unknown_window".to_string(),
            label_param: None,
            used_percent: 0.0,
            resets_at: None,
            window_minutes,
        }
    }

    #[test]
    fn quotas_keep_group_order_and_put_short_windows_first() {
        let grouped = vec![
            ("Gemini".to_string(), quota("gemini-weekly", Some(10_080))),
            ("Gemini".to_string(), quota("gemini-5h", Some(300))),
            ("3p".to_string(), quota("3p-weekly", Some(10_080))),
            ("3p".to_string(), quota("3p-unknown", None)),
            ("3p".to_string(), quota("3p-5h", Some(300))),
        ];
        let ids: Vec<String> = order_quotas(grouped).into_iter().map(|quota| quota.id).collect();
        assert_eq!(
            ids,
            vec!["gemini-5h", "gemini-weekly", "3p-5h", "3p-weekly", "3p-unknown"]
        );
    }

    #[test]
    fn common_value_helpers_follow_the_data_model() {
        assert_eq!(clamp_percent(-2.0), 0.0);
        assert_eq!(clamp_percent(42.5), 42.5);
        assert_eq!(clamp_percent(120.0), 100.0);
        assert_eq!(slugify("Fable 4.1 (Weekly)"), "fable-4-1-weekly");
        assert_eq!(quota_label(Some(300)).0, "quota.five_hour");
        assert_eq!(quota_label(Some(10_080)).0, "quota.weekly");
        assert_eq!(title_case_ascii("plus"), "Plus");
    }

    #[test]
    fn process_failures_are_classified_without_exposing_raw_output() {
        let not_logged_in =
            classify_process_failure("Claude", false, "authentication required", "");
        assert_eq!(not_logged_in.status, StatusCode::NotLoggedIn);

        let offline = classify_process_failure("Codex", false, "network unavailable", "");
        assert_eq!(offline.status, StatusCode::Offline);

        let parse_error = classify_process_failure("Claude", true, "", "unexpected json");
        assert_eq!(parse_error.status, StatusCode::ParseError);
        assert!(!not_logged_in.detail.contains("authentication required"));
        assert!(!offline.detail.contains("network unavailable"));
    }
}
