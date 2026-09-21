//! 檔案：settings/changes.rs
//! 用途：比較「儲存前」與「儲存後」的設定，找出哪些地方需要立即套用（FR-9：儲存後立即生效，不用重啟）。
//!       例如更新間隔改了要叫醒排程、顯示方式改了要換視窗大小、服務啟用或 CLI 位置改了要馬上查一次、
//!       語言改了系統匣選單要換語言。
//!       這裡只做比較（純函式），真正的套用動作在 settings_commands.rs。
//! 誰會用到：settings_commands.rs 的 save_settings。

use super::{Settings, SingleProviderSettings};

/// 這次儲存改到了哪些「需要馬上做點事」的設定。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SettingsChanges {
    /// 自動更新間隔改了：叫醒排程，用新間隔重新判斷
    pub refresh_interval: bool,
    /// log 保留天數改了：更新保留天數並清理舊檔
    pub log_retention: bool,
    /// 顯示方式（輪播／並排）改了：換成該模式記住的視窗大小
    pub display_mode: bool,
    /// 介面語言改了：系統匣選單換成新語言（小工具與設定視窗由畫面自己換）
    pub language: bool,
    /// 有服務剛被啟用、或啟用中的服務換了 CLI 位置：馬上查一次，讓使用者看到結果
    pub needs_requery: bool,
}

impl SettingsChanges {
    /// 比較兩份設定。
    pub fn between(before: &Settings, after: &Settings) -> Self {
        let providers_before = [
            &before.providers.claude,
            &before.providers.codex,
            &before.providers.antigravity,
        ];
        let providers_after = [
            &after.providers.claude,
            &after.providers.codex,
            &after.providers.antigravity,
        ];
        let mut needs_requery = false;
        for index in 0..providers_after.len() {
            if provider_needs_requery(providers_before[index], providers_after[index]) {
                needs_requery = true;
            }
        }

        Self {
            refresh_interval: before.general.refresh_interval_minutes
                != after.general.refresh_interval_minutes,
            log_retention: before.general.log_retention_days != after.general.log_retention_days,
            display_mode: before.display.mode != after.display.mode,
            language: before.general.language != after.general.language,
            needs_requery,
        }
    }
}

/// 單一服務：剛從停用變成啟用，或啟用中換了 CLI 位置，就要重查。
fn provider_needs_requery(before: &SingleProviderSettings, after: &SingleProviderSettings) -> bool {
    if !after.enabled {
        return false;
    }
    let just_enabled = !before.enabled;
    let path_changed = before.cli_path != after.cli_path;
    just_enabled || path_changed
}

#[cfg(test)]
mod tests {
    use super::SettingsChanges;
    use crate::settings::default_settings;

    #[test]
    fn nothing_changed_means_nothing_to_apply() {
        let settings = default_settings().expect("預設設定應有效");
        let changes = SettingsChanges::between(&settings, &settings);
        assert_eq!(changes, SettingsChanges::default());
    }

    #[test]
    fn detects_interval_retention_and_mode_changes() {
        let before = default_settings().expect("預設設定應有效");
        let mut after = before.clone();
        after.general.refresh_interval_minutes = 1;
        after.general.log_retention_days = 30;
        after.display.mode = "overview".to_string();
        after.general.language = "en".to_string();
        let changes = SettingsChanges::between(&before, &after);
        assert!(changes.refresh_interval);
        assert!(changes.log_retention);
        assert!(changes.display_mode);
        assert!(changes.language);
        assert!(!changes.needs_requery);
    }

    #[test]
    fn enabling_a_provider_or_changing_its_cli_path_needs_a_requery() {
        let mut before = default_settings().expect("預設設定應有效");
        before.providers.codex.enabled = false;
        let mut after = before.clone();
        after.providers.codex.enabled = true;
        assert!(SettingsChanges::between(&before, &after).needs_requery);

        let before = default_settings().expect("預設設定應有效");
        let mut after = before.clone();
        after.providers.claude.cli_path = "C:\\Tools\\claude.exe".to_string();
        assert!(SettingsChanges::between(&before, &after).needs_requery);
    }

    #[test]
    fn disabling_a_provider_or_changing_visible_quotas_does_not_requery() {
        let before = default_settings().expect("預設設定應有效");
        let mut after = before.clone();
        after.providers.antigravity.enabled = false;
        after.providers.claude.visible_quotas = vec!["claude.five_hour".to_string()];
        assert!(!SettingsChanges::between(&before, &after).needs_requery);
    }
}
