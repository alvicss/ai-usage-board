//! 檔案：scheduler/tests.rs
//! 用途：scheduler/rules.rs 的單元測試：更新間隔、重置時間補查、略過停用與保險絲、合併結果、保險絲紀錄。
//! 誰會用到：`cargo test`；時間都由參數傳入，不需要真的等待，也不執行任何 CLI。

use super::rules::*;
use crate::cache::empty_snapshot;
use crate::providers::{ProviderError, ProviderId, Quota, StatusCode};
use crate::runtime_state::RuntimeState;
use crate::settings::default_settings;

fn sample_quota(id: &str, used_percent: f64, resets_at: Option<i64>) -> Quota {
    Quota {
        id: id.to_string(),
        label_key: "quota.five_hour".to_string(),
        label_param: None,
        used_percent,
        resets_at,
        window_minutes: Some(300),
    }
}

#[test]
fn interval_uses_last_attempt_time() {
    // 從沒查過：馬上查
    assert!(interval_due(None, 10, 1_000));
    // 上次嘗試 9 分 59 秒前：還不用查
    assert!(!interval_due(Some(1_000), 10, 1_000 + 599));
    // 剛好 10 分鐘：該查了
    assert!(interval_due(Some(1_000), 10, 1_000 + 600));
}

#[test]
fn latest_attempt_picks_the_newest_provider() {
    let settings = default_settings().expect("預設設定應有效");
    let mut snapshot = empty_snapshot(&settings);
    assert_eq!(latest_attempt(&snapshot), None);
    snapshot.providers[0].last_attempt_at = Some(100);
    snapshot.providers[2].last_attempt_at = Some(300);
    assert_eq!(latest_attempt(&snapshot), Some(300));
}

#[test]
fn expired_reset_triggers_only_once() {
    let settings = default_settings().expect("預設設定應有效");
    let mut snapshot = empty_snapshot(&settings);
    snapshot.providers[0]
        .quotas
        .push(sample_quota("claude.five_hour", 50.0, Some(500)));

    // 還沒到重置時間
    assert_eq!(next_unhandled_reset(&snapshot, &settings, &[], 499), None);
    // 過了重置時間、還沒處理過：觸發
    assert_eq!(next_unhandled_reset(&snapshot, &settings, &[], 500), Some(500));
    // 已經處理過同一個重置時間：不再觸發
    assert_eq!(next_unhandled_reset(&snapshot, &settings, &[500], 900), None);
}

#[test]
fn disabled_provider_reset_is_ignored() {
    let mut settings = default_settings().expect("預設設定應有效");
    settings.providers.claude.enabled = false;
    let mut snapshot = empty_snapshot(&settings);
    snapshot.providers[0]
        .quotas
        .push(sample_quota("claude.five_hour", 50.0, Some(500)));
    assert_eq!(next_unhandled_reset(&snapshot, &settings, &[], 900), None);
}

#[test]
fn handled_resets_are_pruned() {
    let settings = default_settings().expect("預設設定應有效");
    let mut snapshot = empty_snapshot(&settings);
    snapshot.providers[1]
        .quotas
        .push(sample_quota("codex.primary", 10.0, Some(700)));
    let mut handled = vec![500, 700];
    prune_handled_resets(&mut handled, &snapshot);
    assert_eq!(handled, vec![700]);
}

#[test]
fn jobs_skip_disabled_and_fuse_tripped_providers() {
    let mut settings = default_settings().expect("預設設定應有效");
    settings.providers.codex.enabled = false;
    let jobs = build_jobs(&settings, true);

    assert_eq!(jobs.len(), 3);
    assert!(jobs[0].skip_reason.is_none(), "Claude 應照常查詢");
    let codex_skip = jobs[1].skip_reason.as_ref().expect("Codex 已停用");
    assert_eq!(codex_skip.status, StatusCode::Disabled);
    let agy_skip = jobs[2].skip_reason.as_ref().expect("保險絲跳脫後不查");
    assert_eq!(agy_skip.status, StatusCode::FuseTripped);
}

#[test]
fn failed_provider_keeps_old_quotas_and_others_update() {
    let settings = default_settings().expect("預設設定應有效");
    let mut snapshot = empty_snapshot(&settings);
    snapshot.providers[0]
        .quotas
        .push(sample_quota("claude.five_hour", 25.0, None));
    snapshot.providers[0].last_success_at = Some(50);

    let mut codex_usage = snapshot.providers[1].clone();
    codex_usage.status = StatusCode::Ok;
    codex_usage.quotas = vec![sample_quota("codex.primary", 40.0, None)];

    let outcomes = vec![
        ProviderOutcome {
            id: ProviderId::Claude,
            attempted: true,
            result: Err(ProviderError::new(StatusCode::Offline, "離線")),
        },
        ProviderOutcome {
            id: ProviderId::Codex,
            attempted: true,
            result: Ok(codex_usage),
        },
    ];
    merge_outcomes(&mut snapshot, &outcomes, &settings, 123);

    let claude = &snapshot.providers[0];
    assert_eq!(claude.status, StatusCode::Offline);
    assert_eq!(claude.quotas[0].used_percent, 25.0, "失敗時保留舊數字");
    assert_eq!(claude.last_attempt_at, Some(123));
    assert_eq!(claude.last_success_at, Some(50), "失敗不更新上次成功時間");

    let codex = &snapshot.providers[1];
    assert_eq!(codex.status, StatusCode::Ok);
    assert_eq!(codex.quotas[0].used_percent, 40.0);
    assert_eq!(codex.last_success_at, Some(123));
}

#[test]
fn fuse_trip_is_recorded_once_with_num_turns() {
    let mut state = RuntimeState::default();
    let tripped = ProviderOutcome {
        id: ProviderId::Antigravity,
        attempted: true,
        result: Err(
            ProviderError::new(StatusCode::FuseTripped, "num_turns=1").with_num_turns(1.0),
        ),
    };

    let outcomes = vec![tripped];

    let newly = update_runtime_state(&mut state, &outcomes, 999);
    assert!(newly);
    assert!(state.antigravity_fuse.tripped);
    assert_eq!(state.antigravity_fuse.tripped_at, Some(999));
    assert_eq!(state.antigravity_fuse.num_turns, Some(1.0));

    // 第二次不應改掉第一次的跳脫時間
    let again = update_runtime_state(&mut state, &outcomes, 2_000);
    assert!(!again);
    assert_eq!(state.antigravity_fuse.tripped_at, Some(999));
}
