//! 檔案：cache.rs
//! 用途：定義 cache.json 的內容（UsageSnapshot，見 41「快取檔」），並負責讀取與安全寫入。
//!       程式啟動時先讀它立即顯示（FR-2）；每輪查詢後整份覆寫（先寫暫存檔再改名）。
//! 誰會用到：lib.rs（啟動時讀取）、scheduler（每輪查詢後寫入）、commands.rs（get_snapshot）。

use crate::atomic_file;
use crate::providers::{ProviderId, ProviderUsage, StatusCode};
use crate::settings::{Settings, SingleProviderSettings};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// cache.json 的格式版本；和程式不符時捨棄舊快取。
pub const CACHE_SCHEMA_VERSION: u32 = 1;

/// 一次完整的用量資料（cache.json 的內容、usage-updated 事件的內容）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UsageSnapshot {
    pub schema_version: u32,
    /// 最後一輪查詢結束時間（epoch 秒）
    pub updated_at: Option<i64>,
    /// 固定順序：claude、codex、antigravity
    pub providers: Vec<ProviderUsage>,
}

/// 還沒有任何資料時的空白快取：三家都是「查詢中…」（停用的是 disabled）。
pub fn empty_snapshot(settings: &Settings) -> UsageSnapshot {
    let entries = [
        (ProviderId::Claude, "Claude"),
        (ProviderId::Codex, "Codex"),
        (ProviderId::Antigravity, "Antigravity"),
    ];
    let mut providers = Vec::new();
    for (id, name) in entries {
        let enabled = SingleProviderSettings::for_id(&settings.providers, id).enabled;
        providers.push(empty_provider(id, name, enabled));
    }

    UsageSnapshot {
        schema_version: CACHE_SCHEMA_VERSION,
        updated_at: None,
        providers,
    }
}

/// 讀取 cache.json；檔案不存在、壞掉或版本不符時回傳空白快取（不讓程式當掉）。
/// 讀到的資料依目前設定重新標記每家是否啟用，並固定成三家的順序。
pub fn load(path: &Path, settings: &Settings) -> UsageSnapshot {
    let mut normalized = empty_snapshot(settings);

    let Ok(text) = fs::read_to_string(path) else {
        return normalized;
    };
    let cached = match serde_json::from_str::<UsageSnapshot>(&text) {
        Ok(cached) => cached,
        Err(_) => {
            log::warn!("cache.json 無法解析，本次使用空白快取");
            return normalized;
        }
    };
    if cached.schema_version != CACHE_SCHEMA_VERSION {
        log::warn!("cache.json 格式版本不相容，本次使用空白快取");
        return normalized;
    }

    normalized.updated_at = cached.updated_at;
    for target in &mut normalized.providers {
        let Some(source) = cached.providers.iter().find(|item| item.id == target.id) else {
            continue;
        };
        let enabled = target.enabled;
        *target = source.clone();
        target.enabled = enabled;
        if !enabled {
            target.status = StatusCode::Disabled;
        }
    }
    normalized
}

/// 安全寫入 cache.json（先寫暫存檔再改名，FR-2）。
pub fn write_atomic(path: &Path, snapshot: &UsageSnapshot) -> Result<(), std::io::Error> {
    let mut bytes = serde_json::to_vec_pretty(snapshot).map_err(std::io::Error::other)?;
    bytes.push(b'\n');
    atomic_file::write_atomic(path, &bytes)
}

/// 建立一家空白的用量資料。
fn empty_provider(id: ProviderId, name: &str, enabled: bool) -> ProviderUsage {
    let status = if enabled {
        StatusCode::Pending
    } else {
        StatusCode::Disabled
    };
    ProviderUsage {
        id,
        name: name.to_string(),
        enabled,
        plan: None,
        status,
        status_detail: None,
        last_attempt_at: None,
        last_success_at: None,
        quotas: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::{empty_snapshot, load, write_atomic};
    use crate::providers::{Quota, StatusCode};
    use crate::settings::default_settings;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_directory(name: &str) -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("系統時間應有效")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("ai-usage-cache-{name}-{unique}"));
        std::fs::create_dir_all(&directory).expect("應建立測試資料夾");
        directory
    }

    #[test]
    fn cache_round_trip_keeps_fixed_provider_order() {
        let directory = temporary_directory("round-trip");
        let path = directory.join("cache.json");
        let settings = default_settings().expect("預設設定應有效");
        let mut snapshot = empty_snapshot(&settings);
        snapshot.updated_at = Some(1_700_000_000);
        snapshot.providers[1].quotas.push(Quota {
            id: "codex.primary".to_string(),
            label_key: "quota.five_hour".to_string(),
            label_param: None,
            used_percent: 42.5,
            resets_at: Some(1_700_003_600),
            window_minutes: Some(300),
        });

        write_atomic(&path, &snapshot).expect("應原子寫入快取");
        assert_eq!(load(&path, &settings), snapshot);
        assert!(!directory.join("cache.json.tmp").exists());
        std::fs::remove_dir_all(directory).expect("應清除測試資料");
    }

    #[test]
    fn broken_or_missing_cache_gives_empty_snapshot() {
        let directory = temporary_directory("broken");
        let path = directory.join("cache.json");
        let settings = default_settings().expect("預設設定應有效");

        // 檔案不存在
        let missing = load(&path, &settings);
        assert_eq!(missing.providers.len(), 3);
        assert_eq!(missing.providers[0].status, StatusCode::Pending);

        // 檔案壞掉
        std::fs::write(&path, "{ not json").expect("應寫入壞檔");
        let broken = load(&path, &settings);
        assert_eq!(broken, empty_snapshot(&settings));
        std::fs::remove_dir_all(directory).expect("應清除測試資料");
    }

    #[test]
    fn disabled_provider_is_marked_when_loading() {
        let directory = temporary_directory("disabled");
        let path = directory.join("cache.json");
        let mut settings = default_settings().expect("預設設定應有效");
        let mut snapshot = empty_snapshot(&settings);
        snapshot.providers[2].status = StatusCode::Ok;
        write_atomic(&path, &snapshot).expect("應寫入快取");

        settings.providers.antigravity.enabled = false;
        let loaded = load(&path, &settings);
        assert!(!loaded.providers[2].enabled);
        assert_eq!(loaded.providers[2].status, StatusCode::Disabled);
        std::fs::remove_dir_all(directory).expect("應清除測試資料");
    }
}
