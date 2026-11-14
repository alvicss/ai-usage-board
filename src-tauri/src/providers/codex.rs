//! 檔案：providers/codex.rs
//! 用途：和 Codex app-server 進行固定 JSON 對話，並解析統一用量格式。
//!       額度順序：主要額度（primary、secondary）在前，額外額度（每個 limitId 一組）在後；組內週期短的在前（order_quotas）。
//!       有 id=2 的 result 但 rateLimits 不存在、是 null、或 primary／secondary 都沒有 → no_quota（帳號沒有訂閱額度資料）；
//!       找不到 id=2 回應、沒有 result 或格式錯亂 → parse_error。
//!       同一個回應裡的 rateLimitResetCredits 是重置券（M-014），解析成 reset_credits；看不懂時只略過重置券。
//! 誰會用到：M-002 查詢入口與後續排程模組。

use super::locate::locate_cli;
use super::process::{ManagedProcess, ProcessError};
use super::{
    clamp_percent, order_quotas, quota_label, title_case_ascii, ProviderError, ProviderId,
    ProviderResult, ProviderUsage, Quota, ResetCredit, ResetCredits, StatusCode,
};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

const ARGUMENTS: &[&str] = &["app-server"];
const INITIALIZED_NOTIFICATION: &str = r#"{"method":"initialized"}"#;
const RATE_LIMITS_REQUEST: &str = r#"{"id":2,"method":"account/rateLimits/read"}"#;

pub fn query(configured_path: &str, timeout: Duration) -> ProviderResult<ProviderUsage> {
    let lines = capture_raw(configured_path, timeout)?;
    let mut usage = parse_response(&lines.join("\n"))?;
    let now = chrono::Utc::now().timestamp();
    usage.last_attempt_at = Some(now);
    usage.last_success_at = Some(now);
    Ok(usage)
}

pub(super) fn capture_raw(configured_path: &str, timeout: Duration) -> ProviderResult<Vec<String>> {
    let cli_path = locate_cli(ProviderId::Codex, configured_path)?;
    let mut process =
        ManagedProcess::spawn(&cli_path, ARGUMENTS, &[]).map_err(map_process_error)?;

    let initialize_request = json!({
        "id": 1,
        "method": "initialize",
        "params": {
            "clientInfo": {
                "name": "ai-usage-board",
                "title": "AI Usage Board",
                "version": env!("CARGO_PKG_VERSION")
            }
        }
    });
    process
        .write_line(&initialize_request.to_string())
        .map_err(map_process_error)?;

    let deadline = Instant::now() + timeout;
    let mut lines = Vec::new();
    let mut initialized = false;
    let mut received_rate_limits = false;

    loop {
        let line = match process.receive_line_until(deadline) {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(ProcessError::Timeout) => {
                let _ = process.terminate();
                return Err(ProviderError::new(StatusCode::Timeout, "Codex 查詢逾時"));
            }
            Err(error) => {
                let _ = process.terminate();
                return Err(map_process_error(error));
            }
        };

        if response_id(&line) == Some(1) && !initialized {
            process
                .write_line(INITIALIZED_NOTIFICATION)
                .and_then(|_| process.write_line(RATE_LIMITS_REQUEST))
                .map_err(map_process_error)?;
            initialized = true;
        }

        received_rate_limits = response_id(&line) == Some(2);
        lines.push(line);
        if received_rate_limits {
            break;
        }
    }

    let completion = process.finish().map_err(map_process_error)?;
    if !received_rate_limits {
        return Err(super::classify_process_failure(
            "Codex",
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
        .find(|value| value.get("id").and_then(Value::as_i64) == Some(2))
        .ok_or_else(|| ProviderError::new(StatusCode::ParseError, "找不到 Codex id=2 回應"))?;

    // id=2 的回應要有 result（有 error 或格式不對就是看不懂）
    let result = response
        .get("result")
        .filter(|result| result.is_object())
        .ok_or_else(|| ProviderError::new(StatusCode::ParseError, "Codex id=2 回應沒有 result"))?;

    // rateLimits 不存在或是 null：CLI 正常回應，但這個帳號沒有訂閱額度資料
    let rate_limits = match result.get("rateLimits") {
        None | Some(Value::Null) => {
            return Err(ProviderError::new(
                StatusCode::NoQuota,
                "Codex 回應沒有 rateLimits（帳號沒有訂閱額度資料）",
            ));
        }
        Some(Value::Object(rate_limits)) => rate_limits,
        Some(_) => {
            return Err(ProviderError::new(
                StatusCode::ParseError,
                "Codex rateLimits 格式不對",
            ));
        }
    };

    let extras = response
        .pointer("/result/rateLimitsByLimitId")
        .or_else(|| response.pointer("/result/rateLimits/rateLimitsByLimitId"));
    if !has_any_window(rate_limits, extras) {
        return Err(ProviderError::new(
            StatusCode::NoQuota,
            "Codex rateLimits 沒有 primary／secondary（帳號沒有訂閱額度資料）",
        ));
    }

    let mut quotas = Vec::new();
    append_window(
        &mut quotas,
        rate_limits.get("primary"),
        "codex.primary",
        None,
    );
    append_window(
        &mut quotas,
        rate_limits.get("secondary"),
        "codex.secondary",
        None,
    );

    append_extra_limits(&mut quotas, extras, rate_limits);

    // 主要額度一組，每個額外額度（limitId）各一組排在後面；組內週期短的在前（5 小時 → 每週）
    let mut grouped_quotas = Vec::new();
    for quota in quotas {
        let group = quota_group(&quota.id);
        grouped_quotas.push((group, quota));
    }
    let quotas = order_quotas(grouped_quotas);

    if quotas.is_empty() {
        return Err(ProviderError::new(
            StatusCode::ParseError,
            "Codex rateLimits 沒有可用額度",
        ));
    }

    let plan = rate_limits
        .get("planType")
        .and_then(Value::as_str)
        .map(title_case_ascii);

    Ok(ProviderUsage {
        id: ProviderId::Codex,
        name: "Codex".to_string(),
        enabled: true,
        plan,
        status: StatusCode::Ok,
        status_detail: None,
        last_attempt_at: None,
        last_success_at: None,
        quotas,
        reset_credits: parse_reset_credits(result.get("rateLimitResetCredits")),
    })
}

/// 解析重置券（M-014，45「Codex」）：result.rateLimitResetCredits = { availableCount, credits: [...] | null }。
/// - 欄位不存在或是 null：這次沒有回報重置券 → None（畫面不顯示重置券）。
/// - credits 是 null：只知道張數、沒有明細；明細也可能比張數少（CLI 說明：後端可能限制明細筆數）。
/// - 只收 status = available 的券；兌換中（redeeming）、已兌換（redeemed）、不明（unknown）都不算。
/// - expiresAt 是 epoch 秒，null 代表不會過期。
///   格式看不懂時只略過重置券（回傳 None），不影響額度的查詢結果。
fn parse_reset_credits(value: Option<&Value>) -> Option<ResetCredits> {
    let summary = value?.as_object()?;
    let available_count = summary.get("availableCount")?.as_u64()?;
    let available_count = u32::try_from(available_count).unwrap_or(u32::MAX);

    let mut credits = Vec::new();
    if let Some(items) = summary.get("credits").and_then(Value::as_array) {
        for item in items {
            let status = item.get("status").and_then(Value::as_str);
            if status != Some("available") {
                continue;
            }
            credits.push(ResetCredit {
                expires_at: item.get("expiresAt").and_then(Value::as_i64),
                title: item
                    .get("title")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            });
        }
    }

    Some(ResetCredits {
        available_count,
        credits,
    })
}

/// rateLimits 裡有沒有任何一個額度視窗：primary 或 secondary 不是 null，或有額外額度（rateLimitsByLimitId 不是空的）。
/// 都沒有 → 帳號沒有訂閱額度資料（no_quota）。
fn has_any_window(rate_limits: &serde_json::Map<String, Value>, extras: Option<&Value>) -> bool {
    for key in ["primary", "secondary"] {
        if let Some(value) = rate_limits.get(key) {
            if !value.is_null() {
                return true;
            }
        }
    }
    if let Some(extras) = extras.and_then(Value::as_object) {
        if !extras.is_empty() {
            return true;
        }
    }
    false
}

/// 額度 ID 屬於哪一組：codex.primary／codex.secondary 是主要額度（"main"）；
/// codex.extra.<limitId>.primary／.secondary 以 limitId 分組。
fn quota_group(quota_id: &str) -> String {
    let Some(rest) = quota_id.strip_prefix("codex.extra.") else {
        return "main".to_string();
    };
    // 去掉最後的 .primary／.secondary，剩下的就是 limitId
    match rest.rsplit_once('.') {
        Some((limit_id, _)) => format!("extra.{limit_id}"),
        None => format!("extra.{rest}"),
    }
}

fn response_id(line: &str) -> Option<i64> {
    serde_json::from_str::<Value>(line)
        .ok()?
        .get("id")?
        .as_i64()
}

fn append_window(
    quotas: &mut Vec<Quota>,
    value: Option<&Value>,
    id: &str,
    label_param_override: Option<String>,
) {
    let Some(value) = value else {
        return;
    };
    let Some(used_percent) = value.get("usedPercent").and_then(Value::as_f64) else {
        return;
    };

    let window_minutes = value
        .get("windowDurationMins")
        .and_then(Value::as_u64)
        .and_then(|minutes| u32::try_from(minutes).ok());
    let (label_key, calculated_param) = quota_label(window_minutes);

    quotas.push(Quota {
        id: id.to_string(),
        label_key: label_key.to_string(),
        label_param: label_param_override.or(calculated_param),
        used_percent: clamp_percent(used_percent),
        resets_at: value.get("resetsAt").and_then(Value::as_i64),
        window_minutes,
    });
}

fn append_extra_limits(
    quotas: &mut Vec<Quota>,
    extras: Option<&Value>,
    base_rate_limits: &serde_json::Map<String, Value>,
) {
    let Some(extras) = extras.and_then(Value::as_object) else {
        return;
    };

    for (limit_id, limit_value) in extras {
        if windows_match_base(limit_value, base_rate_limits) {
            continue;
        }

        let label = Some(limit_id.to_string());
        append_window(
            quotas,
            limit_value.get("primary"),
            &format!("codex.extra.{limit_id}.primary"),
            label.clone(),
        );
        append_window(
            quotas,
            limit_value.get("secondary"),
            &format!("codex.extra.{limit_id}.secondary"),
            label,
        );
    }
}

fn windows_match_base(
    limit_value: &Value,
    base_rate_limits: &serde_json::Map<String, Value>,
) -> bool {
    limit_value.get("primary") == base_rate_limits.get("primary")
        && limit_value.get("secondary") == base_rate_limits.get("secondary")
}

fn map_process_error(error: ProcessError) -> ProviderError {
    let status = if matches!(error, ProcessError::Timeout) {
        StatusCode::Timeout
    } else {
        StatusCode::Error
    };
    ProviderError::new(status, format!("Codex：{error}"))
}

#[cfg(test)]
mod tests {
    use super::{parse_response, quota_group};
    use crate::providers::StatusCode;
    use serde_json::{json, Value};

    /// 把成功 fixture 裡 id=2 回應的 result 交給 change 修改後再轉回文字。
    fn fixture_with_result(change: impl Fn(&mut Value)) -> String {
        let content = include_str!("../../tests/fixtures/codex-ok.jsonl");
        let mut lines = Vec::new();
        for line in content.lines() {
            let mut value: Value = match serde_json::from_str(line) {
                Ok(value) => value,
                Err(_) => {
                    lines.push(line.to_string());
                    continue;
                }
            };
            if value.get("id").and_then(Value::as_i64) == Some(2) {
                if let Some(result) = value.get_mut("result") {
                    change(result);
                }
            }
            lines.push(value.to_string());
        }
        lines.join("\n")
    }

    #[test]
    fn quotas_keep_main_first_and_short_windows_first() {
        let content = include_str!("../../tests/fixtures/codex-ok.jsonl");
        let usage = parse_response(content).expect("Codex fixture 應可解析");
        assert_eq!(usage.quotas[0].id, "codex.primary");
        assert_eq!(usage.quotas[1].id, "codex.secondary");
        for quota in &usage.quotas[2..] {
            assert!(quota.id.starts_with("codex.extra."));
        }
        assert_eq!(quota_group("codex.primary"), "main");
        assert_eq!(quota_group("codex.extra.base_model_inference.secondary"), "extra.base_model_inference");
    }

    #[test]
    fn missing_or_empty_rate_limits_mean_no_quota() {
        let missing = fixture_with_result(|result| {
            result.as_object_mut().expect("result").remove("rateLimits");
            result.as_object_mut().expect("result").remove("rateLimitsByLimitId");
        });
        let error = parse_response(&missing).expect_err("沒有 rateLimits 不能算成功");
        assert_eq!(error.status, StatusCode::NoQuota);

        let empty = fixture_with_result(|result| {
            result["rateLimits"] = json!({ "primary": null, "secondary": null });
            result.as_object_mut().expect("result").remove("rateLimitsByLimitId");
        });
        let error = parse_response(&empty).expect_err("primary／secondary 都沒有不能算成功");
        assert_eq!(error.status, StatusCode::NoQuota);
    }

    #[test]
    fn unreadable_codex_responses_stay_parse_errors() {
        let no_result = fixture_with_result(|result| {
            *result = Value::Null;
        });
        let error = parse_response(&no_result).expect_err("沒有 result 必須失敗");
        assert_eq!(error.status, StatusCode::ParseError);

        let unreadable = fixture_with_result(|result| {
            result["rateLimits"] = json!({ "primary": { "used": 3 } });
            result.as_object_mut().expect("result").remove("rateLimitsByLimitId");
        });
        let error = parse_response(&unreadable).expect_err("看不懂的額度必須失敗");
        assert_eq!(error.status, StatusCode::ParseError);
    }

    #[test]
    fn parses_codex_fixture_and_ignores_notifications() {
        let content = include_str!("../../tests/fixtures/codex-notification-noise.jsonl");
        let usage = parse_response(content).expect("Codex fixture 應可解析");
        assert_eq!(usage.status, StatusCode::Ok);
        assert_eq!(usage.plan.as_deref(), Some("Plus"));
        assert!(usage.quotas.iter().any(|quota| quota.id == "codex.primary"));
        assert!(usage
            .quotas
            .iter()
            .any(|quota| quota.id == "codex.extra.base_model_inference.primary"));
        assert!(!usage
            .quotas
            .iter()
            .any(|quota| quota.id.starts_with("codex.extra.codex.")));
        assert!(usage
            .quotas
            .iter()
            .all(|quota| { quota.used_percent >= 0.0 && quota.used_percent <= 100.0 }));
    }

    #[test]
    fn parses_reset_credits_from_live_fixture() {
        // 2026-11-14 實測（codex-cli 0.167.1）：2 張可用的重置券，各有到期時間
        let content = include_str!("../../tests/fixtures/codex-reset-credits.jsonl");
        let usage = parse_response(content).expect("含重置券的 fixture 應可解析");
        let credits = usage.reset_credits.expect("應有重置券資料");
        assert_eq!(credits.available_count, 2);
        assert_eq!(credits.credits.len(), 2);
        assert_eq!(credits.credits[0].expires_at, Some(1_796_163_567));
        assert_eq!(credits.credits[1].expires_at, Some(1_797_113_812));
        assert_eq!(credits.credits[0].title.as_deref(), Some("Full reset (Weekly + 5 hr)"));
    }

    #[test]
    fn reset_credits_with_zero_count_or_no_detail() {
        // 2026-09 的 fixture：0 張、明細是空清單 → 有回報、但沒有券
        let content = include_str!("../../tests/fixtures/codex-ok.jsonl");
        let usage = parse_response(content).expect("Codex fixture 應可解析");
        let credits = usage.reset_credits.expect("有回報就不是 None");
        assert_eq!(credits.available_count, 0);
        assert!(credits.credits.is_empty());

        // credits 是 null：只知道張數、沒有明細
        let count_only = fixture_with_result(|result| {
            result["rateLimitResetCredits"] = json!({ "availableCount": 3, "credits": null });
        });
        let credits = parse_response(&count_only)
            .expect("應可解析")
            .reset_credits
            .expect("有張數就要回報");
        assert_eq!(credits.available_count, 3);
        assert!(credits.credits.is_empty());
    }

    #[test]
    fn only_available_reset_credits_are_kept() {
        let content = fixture_with_result(|result| {
            result["rateLimitResetCredits"] = json!({
                "availableCount": 1,
                "credits": [
                    { "status": "redeemed", "expiresAt": 1_800_000_000, "title": null },
                    { "status": "available", "expiresAt": null, "title": null },
                    { "status": "redeeming", "expiresAt": 1_800_000_100, "title": null }
                ]
            });
        });
        let credits = parse_response(&content)
            .expect("應可解析")
            .reset_credits
            .expect("應有重置券資料");
        assert_eq!(credits.credits.len(), 1, "只收 status = available");
        assert_eq!(credits.credits[0].expires_at, None, "expiresAt null 代表不會過期");
    }

    #[test]
    fn missing_or_unreadable_reset_credits_do_not_break_quotas() {
        // 舊版 CLI 沒有這個欄位、或是 null：不顯示重置券，額度照常
        let missing = fixture_with_result(|result| {
            result.as_object_mut().expect("result").remove("rateLimitResetCredits");
        });
        let usage = parse_response(&missing).expect("沒有重置券欄位仍要成功");
        assert_eq!(usage.reset_credits, None);
        assert!(!usage.quotas.is_empty());

        // 格式看不懂（例如改版成文字）：只略過重置券
        let unreadable = fixture_with_result(|result| {
            result["rateLimitResetCredits"] = json!("two");
        });
        let usage = parse_response(&unreadable).expect("重置券看不懂不能讓整家失敗");
        assert_eq!(usage.reset_credits, None);
        assert_eq!(usage.status, StatusCode::Ok);
    }
}
