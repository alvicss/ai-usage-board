//! 檔案：scheduler/jobs.rs
//! 用途：一輪查詢裡「三家同時查」的執行細節：每家各開一條執行緒、等全部結束後依固定順序收集結果，
//!       並把每家的結果寫進 log（停用的不查、也不寫查詢紀錄，FR-11；某一家的執行緒意外結束只影響那一家）。
//! 誰會用到：scheduler/mod.rs 的 run_refresh。

use super::rules::{self, ProviderJob, ProviderOutcome};
use crate::providers::{self, ProviderError, ProviderId, StatusCode};
use std::thread;
use std::time::Duration;

/// 同時執行各家的查詢工作，等全部結束後依固定順序回傳結果。
pub fn run_jobs_in_parallel(jobs: Vec<ProviderJob>, timeout: Duration) -> Vec<ProviderOutcome> {
    thread::scope(|scope| {
        let mut handles = Vec::new();
        for job in jobs {
            let handle = scope.spawn(move || run_one_job(job, timeout));
            handles.push(handle);
        }

        let mut outcomes = Vec::new();
        for (index, handle) in handles.into_iter().enumerate() {
            match handle.join() {
                Ok(outcome) => outcomes.push(outcome),
                Err(_) => {
                    // 某一家的執行緒意外結束，只影響那一家
                    let id = rules::PROVIDER_ORDER[index];
                    outcomes.push(ProviderOutcome {
                        id,
                        attempted: true,
                        result: Err(ProviderError::new(StatusCode::Error, "查詢執行緒異常結束")),
                    });
                }
            }
        }
        outcomes
    })
}

/// 執行一家的查詢工作並寫 log（停用的服務不查、也不寫查詢紀錄，FR-11）。
fn run_one_job(job: ProviderJob, timeout: Duration) -> ProviderOutcome {
    let name = provider_name(job.id);

    if let Some(skip_reason) = job.skip_reason {
        if skip_reason.status == StatusCode::FuseTripped {
            log::info!("{name}：保險絲已跳脫，本輪不查詢");
        }
        return ProviderOutcome {
            id: job.id,
            attempted: false,
            result: Err(skip_reason),
        };
    }

    let result = providers::query(job.id, &job.cli_path, timeout);
    match &result {
        Ok(usage) => log::info!("{name} 查詢成功：{} 個額度", usage.quotas.len()),
        Err(error) => log::warn!(
            "{name} 查詢失敗（{}）：{}",
            status_name(error.status),
            error.detail
        ),
    }
    ProviderOutcome {
        id: job.id,
        attempted: true,
        result,
    }
}

/// 服務顯示名稱（log 用）。
fn provider_name(id: ProviderId) -> &'static str {
    match id {
        ProviderId::Claude => "Claude",
        ProviderId::Codex => "Codex",
        ProviderId::Antigravity => "Antigravity",
    }
}

/// 狀態代碼的英文字（log 用，和 cache.json 裡的寫法一致）。
fn status_name(status: StatusCode) -> String {
    serde_json::to_value(status)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "error".to_string())
}
