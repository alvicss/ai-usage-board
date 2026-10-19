//! 檔案：scheduler/rules.rs
//! 用途：排程用到的「純邏輯」：什麼時候該查、哪幾家要查、查完怎麼合併、保險絲怎麼記錄。
//!       這裡的函式都不碰檔案、不碰視窗、時間由參數傳入，所以可以直接寫單元測試。
//! 誰會用到：scheduler/mod.rs。

use crate::cache::UsageSnapshot;
use crate::providers::{ProviderError, ProviderId, ProviderUsage, StatusCode};
use crate::runtime_state::RuntimeState;
use crate::settings::{Settings, SingleProviderSettings};

/// 三家固定的查詢順序（也是 cache.json 裡的順序）。
pub const PROVIDER_ORDER: [ProviderId; 3] =
    [ProviderId::Claude, ProviderId::Codex, ProviderId::Antigravity];

/// 距離「上次嘗試查詢」是否已達更新間隔（FR-2）。
/// 用「上次嘗試」而不是「上次成功」：斷網時不會每 30 秒一直重試。
/// 從來沒查過（None）就代表該馬上查。
pub fn interval_due(last_attempt_at: Option<i64>, interval_minutes: i64, now: i64) -> bool {
    let Some(last_attempt) = last_attempt_at else {
        return true;
    };
    let interval_seconds = interval_minutes * 60;
    now - last_attempt >= interval_seconds
}

/// 從快取找出最近一次嘗試查詢的時間（程式啟動時用來接續計時）。
pub fn latest_attempt(snapshot: &UsageSnapshot) -> Option<i64> {
    let mut latest: Option<i64> = None;
    for provider in &snapshot.providers {
        if let Some(attempt) = provider.last_attempt_at {
            let is_newer = match latest {
                Some(current) => attempt > current,
                None => true,
            };
            if is_newer {
                latest = Some(attempt);
            }
        }
    }
    latest
}

/// 找出「重置時間已過、而且還沒為它補查過」的重置時間（FR-2：同一個重置時間只觸發一次）。
/// 只看啟用中的服務；找不到就回傳 None。
pub fn next_unhandled_reset(
    snapshot: &UsageSnapshot,
    settings: &Settings,
    handled_resets: &[i64],
    now: i64,
) -> Option<i64> {
    for provider in &snapshot.providers {
        let provider_settings = SingleProviderSettings::for_id(&settings.providers, provider.id);
        if !provider_settings.enabled {
            continue;
        }
        for quota in &provider.quotas {
            let Some(reset_at) = quota.resets_at else {
                continue;
            };
            let already_passed = reset_at <= now;
            let already_handled = handled_resets.contains(&reset_at);
            if already_passed && !already_handled {
                return Some(reset_at);
            }
        }
    }
    None
}

/// 只保留快取裡仍存在的重置時間，避免「已處理清單」無限變長。
pub fn prune_handled_resets(handled_resets: &mut Vec<i64>, snapshot: &UsageSnapshot) {
    let mut still_present = Vec::new();
    for reset_at in handled_resets.iter() {
        let mut found = false;
        for provider in &snapshot.providers {
            for quota in &provider.quotas {
                if quota.resets_at == Some(*reset_at) {
                    found = true;
                }
            }
        }
        if found {
            still_present.push(*reset_at);
        }
    }
    *handled_resets = still_present;
}

/// 一家的查詢工作：要查就帶 CLI 路徑；不查（停用、保險絲跳脫）就帶著略過的原因。
#[derive(Debug, Clone)]
pub struct ProviderJob {
    pub id: ProviderId,
    pub cli_path: String,
    pub skip_reason: Option<ProviderError>,
}

/// 依設定與保險絲狀態，決定這一輪三家各自要不要查。
pub fn build_jobs(settings: &Settings, antigravity_fuse_tripped: bool) -> Vec<ProviderJob> {
    let mut jobs = Vec::new();
    for id in PROVIDER_ORDER {
        let provider_settings = SingleProviderSettings::for_id(&settings.providers, id);

        let skip_reason = if !provider_settings.enabled {
            Some(ProviderError::new(StatusCode::Disabled, "服務已停用"))
        } else if id == ProviderId::Antigravity && antigravity_fuse_tripped {
            Some(ProviderError::new(
                StatusCode::FuseTripped,
                "Antigravity 保險絲已跳脫，暫停查詢",
            ))
        } else {
            None
        };

        jobs.push(ProviderJob {
            id,
            cli_path: provider_settings.cli_path.clone(),
            skip_reason,
        });
    }
    jobs
}

/// 一家這一輪的結果：查詢結果，以及是否真的有去查（停用或保險絲略過時為 false）。
#[derive(Debug, Clone)]
pub struct ProviderOutcome {
    pub id: ProviderId,
    pub attempted: bool,
    pub result: Result<ProviderUsage, ProviderError>,
}

/// 把這一輪的結果合併進快取（FR-2）：
/// - 成功：整家換成新資料。
/// - 失敗：保留上次成功的額度數字，只更新狀態與原因。
/// - 停用：標成 disabled，不動舊資料。
pub fn merge_outcomes(
    snapshot: &mut UsageSnapshot,
    outcomes: &[ProviderOutcome],
    settings: &Settings,
    finished_at: i64,
) {
    for outcome in outcomes {
        let enabled = SingleProviderSettings::for_id(&settings.providers, outcome.id).enabled;
        let Some(current) = snapshot
            .providers
            .iter_mut()
            .find(|provider| provider.id == outcome.id)
        else {
            continue;
        };

        match &outcome.result {
            Ok(usage) => {
                *current = usage.clone();
                current.enabled = enabled;
                current.last_attempt_at = Some(finished_at);
                current.last_success_at = Some(finished_at);
            }
            Err(error) => {
                current.enabled = enabled;
                current.status = error.status;
                current.status_detail = Some(error.detail.clone());
                if outcome.attempted {
                    current.last_attempt_at = Some(finished_at);
                }
            }
        }
    }
}

/// 更新 state.json 的內容：保險絲跳脫紀錄。
/// 回傳保險絲是否「這一輪才剛跳脫」（呼叫端要寫一筆警告 log）。
pub fn update_runtime_state(
    state: &mut RuntimeState,
    outcomes: &[ProviderOutcome],
    now: i64,
) -> bool {
    let mut newly_tripped = false;

    for outcome in outcomes {
        // 只有「真的去查了、而且回應觸發保險絲」才記錄；略過的那種是舊的跳脫狀態
        let is_antigravity = outcome.id == ProviderId::Antigravity;
        if !is_antigravity || !outcome.attempted || state.antigravity_fuse.tripped {
            continue;
        }
        if let Err(error) = &outcome.result {
            if error.status == StatusCode::FuseTripped {
                state.antigravity_fuse.tripped = true;
                state.antigravity_fuse.tripped_at = Some(now);
                state.antigravity_fuse.num_turns = error.num_turns;
                newly_tripped = true;
            }
        }
    }

    newly_tripped
}
