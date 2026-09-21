//! 檔案：providers/antigravity.rs
//! 用途：以固定 /usage 指令查詢 Antigravity、換算已用比例並執行保險絲判斷。
//!       額度順序：保持 agy 回報的群組順序，群組內 5 小時在前、每週在後（agy 本身是每週在前，order_quotas 重排）。
//!       保險絲判斷最優先（不是 JSON、格式不對、num_turns 缺少或大於 0 都跳脫）；通過後 groups 裡完全沒有額度才算 no_quota。
//! 誰會用到：M-002 查詢入口與後續排程模組。

use super::locate::locate_cli;
use super::process::{ManagedProcess, ProcessError};
use super::{
    clamp_percent, order_quotas, parse_iso_timestamp, ProviderError, ProviderId, ProviderResult,
    ProviderUsage, Quota, StatusCode,
};
use serde_json::Value;
use std::time::{Duration, Instant};

const ARGUMENTS: &[&str] = &["--print", "/usage", "--output-format", "json"];
const ENVIRONMENT: &[(&str, &str)] = &[("AGY_CLI_DISABLE_AUTO_UPDATE", "true")];

pub fn query(configured_path: &str, timeout: Duration) -> ProviderResult<ProviderUsage> {
    let content = capture_raw(configured_path, timeout)?;
    let mut usage = parse_response(&content)?;
    let now = chrono::Utc::now().timestamp();
    usage.last_attempt_at = Some(now);
    usage.last_success_at = Some(now);
    Ok(usage)
}

pub(super) fn capture_raw(configured_path: &str, timeout: Duration) -> ProviderResult<String> {
    let cli_path = locate_cli(ProviderId::Antigravity, configured_path)?;
    let mut process =
        ManagedProcess::spawn(&cli_path, ARGUMENTS, ENVIRONMENT).map_err(map_process_error)?;
    process.close_stdin();

    let deadline = Instant::now() + timeout;
    let mut lines = Vec::new();

    loop {
        match process.receive_line_until(deadline) {
            Ok(Some(line)) => lines.push(line),
            Ok(None) => break,
            Err(ProcessError::Timeout) => {
                let _ = process.terminate();
                return Err(ProviderError::new(
                    StatusCode::Timeout,
                    "Antigravity 查詢逾時",
                ));
            }
            Err(error) => {
                let _ = process.terminate();
                return Err(map_process_error(error));
            }
        }
    }

    let completion = process.finish().map_err(map_process_error)?;
    let content = lines.join("\n");

    if !completion.status.success() {
        return Err(classify_failed_process(&completion.stderr, &content));
    }

    validate_fuse(&content)?;
    Ok(content)
}

pub fn parse_response(content: &str) -> ProviderResult<ProviderUsage> {
    let root = validate_fuse(content)?;
    let groups = root
        .pointer("/command/data/groups")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            ProviderError::new(
                StatusCode::FuseTripped,
                "Antigravity 回應缺少 command.data.groups，保險絲已跳脫",
            )
        })?;

    // 保險絲通過、groups 裡一個額度都沒有：帳號沒有訂閱額度資料（不是格式錯亂）
    if !has_any_bucket(groups) {
        return Err(ProviderError::new(
            StatusCode::NoQuota,
            "Antigravity 回應沒有任何額度（帳號沒有訂閱額度資料）",
        ));
    }

    // 每個額度記下所屬群組，最後依「群組順序、群組內週期由短到長」排好
    let mut grouped_quotas = Vec::new();

    for group in groups {
        let group_name = group
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Antigravity");
        let Some(buckets) = group.get("buckets").and_then(Value::as_array) else {
            continue;
        };

        for bucket in buckets {
            let Some(bucket_id) = bucket.get("id").and_then(Value::as_str) else {
                continue;
            };
            let Some(remaining_fraction) = bucket.get("remaining_fraction").and_then(Value::as_f64)
            else {
                continue;
            };

            let window = bucket.get("window").and_then(Value::as_str);
            let window_minutes = match window {
                Some("5h") => Some(300),
                Some("weekly") => Some(10_080),
                _ => None,
            };
            let label_key = match window_minutes {
                Some(300) => "quota.five_hour",
                Some(10_080) => "quota.weekly",
                _ => "quota.unknown_window",
            };
            let label_param = if bucket_id.starts_with("gemini") {
                "Gemini".to_string()
            } else if bucket_id.starts_with("3p") {
                "Claude+GPT".to_string()
            } else {
                group_name.to_string()
            };

            let quota = Quota {
                id: format!("antigravity.{bucket_id}"),
                label_key: label_key.to_string(),
                label_param: Some(label_param),
                used_percent: clamp_percent((1.0 - remaining_fraction) * 100.0),
                resets_at: bucket
                    .get("reset_time")
                    .and_then(Value::as_str)
                    .and_then(parse_iso_timestamp),
                window_minutes,
            };
            grouped_quotas.push((group_name.to_string(), quota));
        }
    }

    let quotas = order_quotas(grouped_quotas);
    if quotas.is_empty() {
        return Err(ProviderError::new(
            StatusCode::ParseError,
            "Antigravity groups 沒有可用額度",
        ));
    }

    let plan = root
        .pointer("/command/data/plan")
        .or_else(|| root.get("plan"))
        .and_then(Value::as_str)
        .map(super::title_case_ascii);

    Ok(ProviderUsage {
        id: ProviderId::Antigravity,
        name: "Antigravity".to_string(),
        enabled: true,
        plan,
        status: StatusCode::Ok,
        status_detail: None,
        last_attempt_at: None,
        last_success_at: None,
        quotas,
    })
}

/// groups 裡有沒有任何一個 bucket（額度）。groups 是空陣列、或每個群組的 buckets 都是空的，就是沒有。
fn has_any_bucket(groups: &[Value]) -> bool {
    for group in groups {
        if let Some(buckets) = group.get("buckets").and_then(Value::as_array) {
            if !buckets.is_empty() {
                return true;
            }
        }
    }
    false
}

fn validate_fuse(content: &str) -> ProviderResult<Value> {
    let root = serde_json::from_str::<Value>(content).map_err(|_| {
        ProviderError::new(
            StatusCode::FuseTripped,
            "Antigravity 正常結束但未回傳 JSON，保險絲已跳脫",
        )
    })?;

    if root.pointer("/command/data/groups").is_none() {
        return Err(ProviderError::new(
            StatusCode::FuseTripped,
            "Antigravity 回應不是預期的用量格式，保險絲已跳脫",
        ));
    }

    let num_turns = find_number_by_key(&root, "num_turns").ok_or_else(|| {
        ProviderError::new(
            StatusCode::FuseTripped,
            "Antigravity 回應缺少 num_turns，無法確認零消耗",
        )
    })?;

    if num_turns > 0.0 {
        return Err(ProviderError::new(
            StatusCode::FuseTripped,
            format!("Antigravity 回報 num_turns={num_turns}，保險絲已跳脫"),
        )
        .with_num_turns(num_turns));
    }

    Ok(root)
}

fn find_number_by_key(value: &Value, target_key: &str) -> Option<f64> {
    match value {
        Value::Object(object) => {
            if let Some(number) = object.get(target_key).and_then(Value::as_f64) {
                return Some(number);
            }
            object
                .values()
                .find_map(|child| find_number_by_key(child, target_key))
        }
        Value::Array(array) => array
            .iter()
            .find_map(|child| find_number_by_key(child, target_key)),
        _ => None,
    }
}

fn classify_failed_process(stderr: &str, stdout: &str) -> ProviderError {
    let combined = format!("{stderr}\n{stdout}").to_ascii_lowercase();
    if combined.contains("unknown option")
        || combined.contains("output-format")
        || combined.contains("unknown command")
    {
        return ProviderError::new(
            StatusCode::Outdated,
            "Antigravity CLI 不支援 /usage JSON，請更新版本",
        );
    }

    super::classify_process_failure("Antigravity", false, stderr, stdout)
}

fn map_process_error(error: ProcessError) -> ProviderError {
    let status = if matches!(error, ProcessError::Timeout) {
        StatusCode::Timeout
    } else {
        StatusCode::Error
    };
    ProviderError::new(status, format!("Antigravity：{error}"))
}

#[cfg(test)]
mod tests {
    use super::parse_response;
    use crate::providers::StatusCode;
    use serde_json::Value;

    /// 讀成功的 fixture，交給 change 修改後再轉回文字（用來產生「缺欄位」的測試資料）。
    fn modified_fixture(change: impl Fn(&mut Value)) -> String {
        let content = include_str!("../../tests/fixtures/antigravity-ok.json");
        let mut root: Value = serde_json::from_str(content).expect("fixture 應是 JSON");
        change(&mut root);
        root.to_string()
    }

    #[test]
    fn quotas_are_ordered_five_hours_first_within_each_group() {
        let content = include_str!("../../tests/fixtures/antigravity-ok.json");
        let usage = parse_response(content).expect("Antigravity fixture 應可解析");
        let ids: Vec<&str> = usage.quotas.iter().map(|quota| quota.id.as_str()).collect();
        // agy 回報的順序是每週在前；畫面要 5 小時在前、每週在後，群組順序不變
        assert_eq!(
            ids,
            vec![
                "antigravity.gemini-5h",
                "antigravity.gemini-weekly",
                "antigravity.3p-5h",
                "antigravity.3p-weekly"
            ]
        );
    }

    #[test]
    fn empty_groups_mean_no_quota_but_the_fuse_still_comes_first() {
        let empty_groups = modified_fixture(|root| {
            root["command"]["data"]["groups"] = Value::Array(Vec::new());
        });
        let error = parse_response(&empty_groups).expect_err("沒有額度不能算成功");
        assert_eq!(error.status, StatusCode::NoQuota);

        let empty_buckets = modified_fixture(|root| {
            for group in root["command"]["data"]["groups"].as_array_mut().expect("groups") {
                group["buckets"] = Value::Array(Vec::new());
            }
        });
        let error = parse_response(&empty_buckets).expect_err("沒有額度不能算成功");
        assert_eq!(error.status, StatusCode::NoQuota);

        // 保險絲優先：groups 是空的但 num_turns > 0，仍然要跳保險絲
        let turns_and_empty = modified_fixture(|root| {
            root["command"]["data"]["groups"] = Value::Array(Vec::new());
            root["num_turns"] = Value::from(1);
        });
        let error = parse_response(&turns_and_empty).expect_err("num_turns > 0 必須跳保險絲");
        assert_eq!(error.status, StatusCode::FuseTripped);
    }

    #[test]
    fn buckets_without_numbers_are_still_a_parse_error() {
        // 有額度但格式看不懂（可能是 CLI 改版）：維持 parse_error，不能誤判成沒有訂閱
        let broken = modified_fixture(|root| {
            for group in root["command"]["data"]["groups"].as_array_mut().expect("groups") {
                for bucket in group["buckets"].as_array_mut().expect("buckets") {
                    bucket.as_object_mut().expect("bucket").remove("remaining_fraction");
                }
            }
        });
        let error = parse_response(&broken).expect_err("看不懂的額度不能算成功");
        assert_eq!(error.status, StatusCode::ParseError);
    }

    #[test]
    fn parses_remaining_fraction_as_used_percent() {
        let content = include_str!("../../tests/fixtures/antigravity-ok.json");
        let usage = parse_response(content).expect("Antigravity fixture 應可解析");
        assert_eq!(usage.status, StatusCode::Ok);
        assert_eq!(usage.plan, None);
        assert_eq!(usage.quotas.len(), 4);
        assert!(usage
            .quotas
            .iter()
            .any(|quota| quota.id == "antigravity.gemini-5h"));
        assert!(usage
            .quotas
            .iter()
            .all(|quota| { quota.used_percent >= 0.0 && quota.used_percent <= 100.0 }));
    }

    #[test]
    fn num_turns_above_zero_trips_fuse() {
        let content = include_str!("../../tests/fixtures/antigravity-turns.json");
        let error = parse_response(content).expect_err("num_turns > 0 必須跳保險絲");
        assert_eq!(error.status, StatusCode::FuseTripped);
        assert_eq!(error.num_turns, Some(1.0));
    }

    #[test]
    fn plain_text_trips_fuse() {
        let content = include_str!("../../tests/fixtures/antigravity-plain-text.txt");
        let error = parse_response(content).expect_err("非 JSON 必須跳保險絲");
        assert_eq!(error.status, StatusCode::FuseTripped);
    }
}
