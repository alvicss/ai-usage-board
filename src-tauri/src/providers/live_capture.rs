//! 檔案：providers/live_capture.rs
//! 用途：以明確忽略測試擷取三家實際回應，先去識別化再寫成單元測試 fixtures。
//! 誰會用到：M-002 開發與 CLI 改版後的維護人員；一般測試不會執行真實 CLI。

use super::{antigravity, claude, codex};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const LIVE_QUERY_TIMEOUT: Duration = Duration::from_secs(45);
const FIXTURE_NOTE: &str = "此檔案為去識別化 CLI 實測資料，只保留用量解析需要的欄位。";

#[test]
#[ignore = "會執行三家已登入的官方 CLI；只在 M-002 明確實測時手動執行"]
fn capture_sanitized_live_fixtures() {
    let fixture_directory = fixture_directory();
    fs::create_dir_all(&fixture_directory).expect("應建立 fixtures 資料夾");

    let claude_lines = claude::capture_raw("", LIVE_QUERY_TIMEOUT).expect("Claude 實測查詢應成功");
    // initialize 回應含大量環境中繼資料；fixture 只保留解析器真正需要的 usage 回應。
    let claude_sanitized = sanitize_json_lines(&only_claude_usage_lines(&claude_lines));
    assert_sanitized(&claude_sanitized);
    write_fixture(
        &fixture_directory.join("claude-ok.jsonl"),
        &claude_sanitized,
    );
    write_fixture(
        &fixture_directory.join("claude-missing-fields.jsonl"),
        &claude_without_rate_limits(&claude_sanitized),
    );

    let codex_lines = codex::capture_raw("", LIVE_QUERY_TIMEOUT).expect("Codex 實測查詢應成功");
    let codex_sanitized = sanitize_json_lines(&codex_lines);
    assert_sanitized(&codex_sanitized);
    write_fixture(&fixture_directory.join("codex-ok.jsonl"), &codex_sanitized);
    let codex_with_noise = format!(
        "{}\n{}\n{}",
        fixture_note_line(),
        r#"{"method":"fixture/notification","params":{"safe":true}}"#,
        codex_sanitized
    );
    write_fixture(
        &fixture_directory.join("codex-notification-noise.jsonl"),
        &codex_with_noise,
    );

    let antigravity_raw =
        antigravity::capture_raw("", LIVE_QUERY_TIMEOUT).expect("Antigravity 實測查詢應成功");
    let mut antigravity_value =
        serde_json::from_str::<Value>(&antigravity_raw).expect("Antigravity 應回傳 JSON");
    sanitize_value(&mut antigravity_value);
    antigravity_value
        .as_object_mut()
        .expect("Antigravity 根節點應為物件")
        .insert("_fixture_note".to_string(), json!(FIXTURE_NOTE));
    let antigravity_sanitized =
        serde_json::to_string_pretty(&antigravity_value).expect("應序列化 fixture");
    assert_sanitized(&antigravity_sanitized);
    write_fixture(
        &fixture_directory.join("antigravity-ok.json"),
        &antigravity_sanitized,
    );

    let mut turns_value = antigravity_value;
    assert!(
        set_number_by_key(&mut turns_value, "num_turns", 1.0),
        "實測回應應包含 num_turns"
    );
    let turns_fixture = serde_json::to_string_pretty(&turns_value).expect("應序列化保險絲 fixture");
    write_fixture(
        &fixture_directory.join("antigravity-turns.json"),
        &turns_fixture,
    );
    write_fixture(
        &fixture_directory.join("antigravity-plain-text.txt"),
        "此檔案是保險絲測試資料：模擬 CLI 回傳非 JSON 文字。\nnot-json",
    );
}

#[test]
#[ignore = "會執行三家已登入的官方 CLI；只在 M-002 明確實測時手動執行"]
fn query_all_providers_as_unified_data() {
    let providers = vec![
        claude::query("", LIVE_QUERY_TIMEOUT).expect("Claude 統一格式查詢應成功"),
        codex::query("", LIVE_QUERY_TIMEOUT).expect("Codex 統一格式查詢應成功"),
        antigravity::query("", LIVE_QUERY_TIMEOUT).expect("Antigravity 統一格式查詢應成功"),
    ];

    assert!(providers
        .iter()
        .all(|provider| provider.last_success_at.is_some()));
    println!(
        "{}",
        serde_json::to_string_pretty(&providers).expect("統一格式應可序列化")
    );
}

#[test]
#[ignore = "會平行執行三家已登入的官方 CLI 共 10 輪；用於 Job Object 殘留行程驗證"]
fn repeated_queries_finish_ten_rounds() {
    for round in 1..=10 {
        std::thread::scope(|scope| {
            let claude_handle = scope.spawn(|| claude::query("", LIVE_QUERY_TIMEOUT));
            let codex_handle = scope.spawn(|| codex::query("", LIVE_QUERY_TIMEOUT));
            let antigravity_handle = scope.spawn(|| antigravity::query("", LIVE_QUERY_TIMEOUT));

            claude_handle
                .join()
                .expect("Claude 查詢 thread 不應 panic")
                .expect("Claude 查詢應成功");
            codex_handle
                .join()
                .expect("Codex 查詢 thread 不應 panic")
                .expect("Codex 查詢應成功");
            antigravity_handle
                .join()
                .expect("Antigravity 查詢 thread 不應 panic")
                .expect("Antigravity 查詢應成功");
        });
        println!("M-002 行程清理實測：第 {round}/10 輪完成");
    }
}

fn fixture_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn only_claude_usage_lines(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .filter(|line| {
            serde_json::from_str::<Value>(line)
                .ok()
                .and_then(|value| value.pointer("/response/request_id").cloned())
                .and_then(|value| value.as_str().map(str::to_owned))
                .as_deref()
                == Some("usage")
        })
        .cloned()
        .collect()
}

fn sanitize_json_lines(lines: &[String]) -> String {
    let mut sanitized_lines = vec![fixture_note_line()];

    for line in lines {
        let mut value = match serde_json::from_str::<Value>(line) {
            Ok(value) => value,
            Err(_) => json!({
                "type": "fixture_non_json_line",
                "description": "原始非 JSON 行已移除"
            }),
        };
        sanitize_value(&mut value);
        sanitized_lines.push(serde_json::to_string(&value).expect("去識別化後的 JSON 應可序列化"));
    }

    sanitized_lines.join("\n")
}

fn fixture_note_line() -> String {
    json!({
        "type": "fixture_note",
        "description": FIXTURE_NOTE
    })
    .to_string()
}

fn sanitize_value(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if is_sensitive_key(key) {
                    *child = Value::String("<redacted>".to_string());
                    continue;
                }
                sanitize_value(child);
            }
        }
        Value::Array(array) => {
            for child in array {
                sanitize_value(child);
            }
        }
        Value::String(text) => {
            let lowercase = text.to_ascii_lowercase();
            if text.contains('@') {
                *text = "<redacted-email>".to_string();
            } else if lowercase.contains("\\users\\") || lowercase.starts_with("c:/users/") {
                *text = "<redacted-path>".to_string();
            }
        }
        _ => {}
    }
}

fn is_sensitive_key(key: &str) -> bool {
    matches!(
        key.to_ascii_lowercase().as_str(),
        "email"
            | "account_id"
            | "accountid"
            | "conversation_id"
            | "conversationid"
            | "device_id"
            | "deviceid"
            | "installation_id"
            | "installationid"
            | "machine_id"
            | "machineid"
            | "organization"
            | "organization_id"
            | "organizationid"
            | "org_id"
            | "server_name"
            | "servername"
            | "userid"
            | "user_id"
            | "session_id"
            | "sessionid"
            | "cwd"
            | "home_dir"
            | "api_key"
            | "auth_token"
            | "access_token"
            | "refresh_token"
    )
}

fn claude_without_rate_limits(content: &str) -> String {
    let mut output = Vec::new();

    for line in content.lines() {
        let mut value = serde_json::from_str::<Value>(line).expect("fixture 每行應為 JSON");
        if value
            .pointer("/response/request_id")
            .and_then(Value::as_str)
            == Some("usage")
        {
            if let Some(response) = value
                .pointer_mut("/response/response")
                .and_then(Value::as_object_mut)
            {
                response.remove("rate_limits");
            }
        }
        output.push(serde_json::to_string(&value).expect("應序列化 fixture"));
    }

    output.join("\n")
}

fn set_number_by_key(value: &mut Value, target_key: &str, new_value: f64) -> bool {
    match value {
        Value::Object(object) => {
            if object.contains_key(target_key) {
                object.insert(target_key.to_string(), json!(new_value));
                return true;
            }
            object
                .values_mut()
                .any(|child| set_number_by_key(child, target_key, new_value))
        }
        Value::Array(array) => array
            .iter_mut()
            .any(|child| set_number_by_key(child, target_key, new_value)),
        _ => false,
    }
}

fn assert_sanitized(content: &str) {
    let lowercase = content.to_ascii_lowercase();
    assert!(!content.contains('@'), "fixture 不得包含 email");
    assert!(
        !lowercase.contains("\\users\\") && !lowercase.contains("c:/users/"),
        "fixture 不得包含使用者目錄"
    );
}

fn write_fixture(path: &Path, content: &str) {
    fs::write(path, format!("{content}\n")).expect("應寫入 fixture");
}
