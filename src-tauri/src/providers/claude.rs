//! 檔案：providers/claude.rs
//! 用途：用固定 control_request 查詢 Claude 用量，並解析成統一格式。
//!       額度順序：5 小時、每週，再來是單一模型的每週額度（order_quotas）。
//!       有 usage 回應但 rate_limits 不存在、是 null、或 five_hour／seven_day／model_scoped 三塊都沒有 → no_quota
//!       （帳號沒有訂閱額度資料，例如改用 API 金鑰按量付費）；找不到 usage 回應或格式錯亂 → parse_error。
//! 誰會用到：M-002 查詢入口與後續排程模組。

use super::locate::locate_cli;
use super::process::{ManagedProcess, ProcessError};
use super::{
    clamp_percent, order_quotas, parse_iso_timestamp, slugify, ProviderError, ProviderId,
    ProviderResult, ProviderUsage, Quota, StatusCode,
};
use serde_json::Map;
use serde_json::Value;
use std::time::{Duration, Instant};

const ARGUMENTS: &[&str] = &[
    "-p",
    "--input-format",
    "stream-json",
    "--output-format",
    "stream-json",
    "--verbose",
    "--setting-sources",
    "",
    "--strict-mcp-config",
    "--no-session-persistence",
];
const INITIALIZE_REQUEST: &str =
    r#"{"type":"control_request","request_id":"init","request":{"subtype":"initialize"}}"#;
const USAGE_REQUEST: &str = r#"{"type":"control_request","request_id":"usage","request":{"subtype":"get_usage","skip_behaviors":true}}"#;

pub fn query(configured_path: &str, timeout: Duration) -> ProviderResult<ProviderUsage> {
    let lines = capture_raw(configured_path, timeout)?;
    let mut usage = parse_response(&lines.join("\n"))?;
    let now = chrono::Utc::now().timestamp();
    usage.last_attempt_at = Some(now);
    usage.last_success_at = Some(now);
    Ok(usage)
}

pub(super) fn capture_raw(configured_path: &str, timeout: Duration) -> ProviderResult<Vec<String>> {
    let cli_path = locate_cli(ProviderId::Claude, configured_path)?;
    let mut process =
        ManagedProcess::spawn(&cli_path, ARGUMENTS, &[]).map_err(map_process_error)?;

    process
        .write_line(INITIALIZE_REQUEST)
        .and_then(|_| process.write_line(USAGE_REQUEST))
        .map_err(map_process_error)?;

    let deadline = Instant::now() + timeout;
    let mut lines = Vec::new();
    let mut received_usage = false;

    loop {
        let line = match process.receive_line_until(deadline) {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(ProcessError::Timeout) => {
                let _ = process.terminate();
                return Err(ProviderError::new(StatusCode::Timeout, "Claude 查詢逾時"));
            }
            Err(error) => {
                let _ = process.terminate();
                return Err(map_process_error(error));
            }
        };

        received_usage = is_usage_response(&line);
        lines.push(line);
        if received_usage {
            break;
        }
    }

    let completion = process.finish().map_err(map_process_error)?;
    if !received_usage {
        return Err(super::classify_process_failure(
            "Claude",
            completion.status.success(),
            &completion.stderr,
            &lines.join("\n"),
        ));
    }

    Ok(lines)
}

pub fn parse_response(content: &str) -> ProviderResult<ProviderUsage> {
    let response = content
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(is_usage_response_value)
        .ok_or_else(|| {
            ProviderError::new(
                StatusCode::ParseError,
                "找不到 Claude usage control_response",
            )
        })?;

    // usage 回應的內容本身要是物件；不是的話（例如 control_response 回報錯誤）就是看不懂
    let body = response
        .pointer("/response/response")
        .and_then(Value::as_object)
        .ok_or_else(|| ProviderError::new(StatusCode::ParseError, "Claude usage 回應沒有內容"))?;

    // rate_limits 不存在或是 null：CLI 正常回應，但這個帳號沒有訂閱額度資料
    let rate_limits = match body.get("rate_limits") {
        None | Some(Value::Null) => {
            return Err(ProviderError::new(
                StatusCode::NoQuota,
                "Claude 回應沒有 rate_limits（帳號沒有訂閱額度資料）",
            ));
        }
        Some(Value::Object(rate_limits)) => rate_limits,
        Some(_) => {
            return Err(ProviderError::new(
                StatusCode::ParseError,
                "Claude rate_limits 格式不對",
            ));
        }
    };
    if !has_any_quota_block(rate_limits) {
        return Err(ProviderError::new(
            StatusCode::NoQuota,
            "Claude rate_limits 沒有任何額度區塊（帳號沒有訂閱額度資料）",
        ));
    }

    let mut quotas = Vec::new();
    append_standard_quota(
        &mut quotas,
        rate_limits.get("five_hour"),
        "claude.five_hour",
        "quota.five_hour",
        300,
    );
    append_standard_quota(
        &mut quotas,
        rate_limits.get("seven_day"),
        "claude.seven_day",
        "quota.weekly",
        10_080,
    );
    append_model_quotas(&mut quotas, rate_limits.get("model_scoped"));

    // 主要額度（5 小時、每週）一組，單一模型額度一組排在後面；組內週期短的在前
    let mut grouped_quotas = Vec::new();
    for quota in quotas {
        let group = if quota.id.starts_with("claude.model.") {
            "model"
        } else {
            "main"
        };
        grouped_quotas.push((group.to_string(), quota));
    }
    let quotas = order_quotas(grouped_quotas);

    if quotas.is_empty() {
        return Err(ProviderError::new(
            StatusCode::ParseError,
            "Claude rate_limits 沒有可用額度",
        ));
    }

    let plan = response
        .pointer("/response/response/subscription_type")
        .and_then(Value::as_str)
        .map(super::title_case_ascii);

    Ok(ProviderUsage {
        id: ProviderId::Claude,
        name: "Claude".to_string(),
        enabled: true,
        plan,
        status: StatusCode::Ok,
        status_detail: None,
        last_attempt_at: None,
        last_success_at: None,
        quotas,
    })
}

/// rate_limits 裡有沒有任何一塊額度資料：five_hour、seven_day 不是 null，或 model_scoped 是非空的清單。
/// 三塊都沒有 → 帳號沒有訂閱額度資料（no_quota）。
fn has_any_quota_block(rate_limits: &Map<String, Value>) -> bool {
    for key in ["five_hour", "seven_day"] {
        if let Some(value) = rate_limits.get(key) {
            if !value.is_null() {
                return true;
            }
        }
    }
    if let Some(models) = rate_limits.get("model_scoped").and_then(Value::as_array) {
        if !models.is_empty() {
            return true;
        }
    }
    false
}

fn is_usage_response(line: &str) -> bool {
    serde_json::from_str::<Value>(line)
        .ok()
        .is_some_and(|value| is_usage_response_value(&value))
}

fn is_usage_response_value(value: &Value) -> bool {
    value.get("type").and_then(Value::as_str) == Some("control_response")
        && value
            .pointer("/response/request_id")
            .and_then(Value::as_str)
            == Some("usage")
}

fn append_standard_quota(
    quotas: &mut Vec<Quota>,
    value: Option<&Value>,
    id: &str,
    label_key: &str,
    window_minutes: u32,
) {
    let Some(value) = value else {
        return;
    };
    let Some(used_percent) = value.get("utilization").and_then(Value::as_f64) else {
        return;
    };

    quotas.push(Quota {
        id: id.to_string(),
        label_key: label_key.to_string(),
        label_param: None,
        used_percent: clamp_percent(used_percent),
        resets_at: value
            .get("resets_at")
            .and_then(Value::as_str)
            .and_then(parse_iso_timestamp),
        window_minutes: Some(window_minutes),
    });
}

fn append_model_quotas(quotas: &mut Vec<Quota>, value: Option<&Value>) {
    let Some(models) = value.and_then(Value::as_array) else {
        return;
    };

    for model in models {
        let Some(display_name) = model.get("display_name").and_then(Value::as_str) else {
            continue;
        };
        let Some(used_percent) = model.get("utilization").and_then(Value::as_f64) else {
            continue;
        };

        quotas.push(Quota {
            id: format!("claude.model.{}", slugify(display_name)),
            label_key: "quota.model_weekly".to_string(),
            label_param: Some(display_name.to_string()),
            used_percent: clamp_percent(used_percent),
            resets_at: model
                .get("resets_at")
                .and_then(Value::as_str)
                .and_then(parse_iso_timestamp),
            window_minutes: Some(10_080),
        });
    }
}

fn map_process_error(error: ProcessError) -> ProviderError {
    let status = if matches!(error, ProcessError::Timeout) {
        StatusCode::Timeout
    } else {
        StatusCode::Error
    };
    ProviderError::new(status, format!("Claude：{error}"))
}

#[cfg(test)]
mod tests {
    use super::parse_response;
    use crate::providers::StatusCode;
    use serde_json::{json, Value};

    #[test]
    fn parses_claude_fixture() {
        let content = include_str!("../../tests/fixtures/claude-ok.jsonl");
        let usage = parse_response(content).expect("Claude fixture 應可解析");
        assert_eq!(usage.status, StatusCode::Ok);
        assert_eq!(usage.plan.as_deref(), Some("Max"));
        assert!(usage
            .quotas
            .iter()
            .any(|quota| quota.id == "claude.five_hour"));
        assert!(usage
            .quotas
            .iter()
            .all(|quota| { quota.used_percent >= 0.0 && quota.used_percent <= 100.0 }));
    }

    #[test]
    fn quotas_are_ordered_five_hours_weekly_then_models() {
        let content = include_str!("../../tests/fixtures/claude-ok.jsonl");
        let usage = parse_response(content).expect("Claude fixture 應可解析");
        assert_eq!(usage.quotas[0].id, "claude.five_hour");
        assert_eq!(usage.quotas[1].id, "claude.seven_day");
        for quota in &usage.quotas[2..] {
            assert!(quota.id.starts_with("claude.model."));
        }
    }

    #[test]
    fn missing_rate_limits_means_no_quota() {
        // 有 usage 回應、只是沒有 rate_limits：帳號沒有訂閱額度資料，不是 CLI 改版
        let content = include_str!("../../tests/fixtures/claude-missing-fields.jsonl");
        let error = parse_response(content).expect_err("缺少 rate_limits 必須失敗");
        assert_eq!(error.status, StatusCode::NoQuota);
    }

    /// 把成功 fixture 裡 usage 回應的 rate_limits 換成指定的值。
    fn fixture_with_rate_limits(rate_limits: Value) -> String {
        let content = include_str!("../../tests/fixtures/claude-ok.jsonl");
        let mut lines = Vec::new();
        for line in content.lines() {
            let mut value: Value = match serde_json::from_str(line) {
                Ok(value) => value,
                Err(_) => {
                    lines.push(line.to_string());
                    continue;
                }
            };
            if let Some(body) = value.pointer_mut("/response/response") {
                if body.get("rate_limits").is_some() {
                    body["rate_limits"] = rate_limits.clone();
                }
            }
            lines.push(value.to_string());
        }
        lines.join("\n")
    }

    #[test]
    fn null_or_empty_rate_limits_mean_no_quota() {
        let null_limits = fixture_with_rate_limits(Value::Null);
        let error = parse_response(&null_limits).expect_err("rate_limits 是 null 不能算成功");
        assert_eq!(error.status, StatusCode::NoQuota);

        let empty_blocks = fixture_with_rate_limits(json!({
            "five_hour": null,
            "seven_day": null,
            "model_scoped": []
        }));
        let error = parse_response(&empty_blocks).expect_err("三塊都沒有不能算成功");
        assert_eq!(error.status, StatusCode::NoQuota);
    }

    #[test]
    fn unreadable_responses_stay_parse_errors() {
        // 找不到 usage 回應
        let error = parse_response("{\"type\":\"system\"}").expect_err("沒有 usage 回應必須失敗");
        assert_eq!(error.status, StatusCode::ParseError);

        // rate_limits 不是物件
        let wrong_type = fixture_with_rate_limits(json!("oops"));
        let error = parse_response(&wrong_type).expect_err("格式不對必須失敗");
        assert_eq!(error.status, StatusCode::ParseError);

        // 有額度區塊但看不懂（沒有 utilization）：可能是 CLI 改版，不能誤判成沒有訂閱
        let unreadable = fixture_with_rate_limits(json!({ "five_hour": { "used": 3 } }));
        let error = parse_response(&unreadable).expect_err("看不懂的額度必須失敗");
        assert_eq!(error.status, StatusCode::ParseError);
    }
}
