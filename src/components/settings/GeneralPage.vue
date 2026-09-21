<!--
  檔案：components/settings/GeneralPage.vue
  用途：設定視窗「一般」頁（43「設定視窗」）：介面語言（自動／繁體中文／English，FR-17）、
        開機自動啟動、自動更新間隔、警示門檻、查詢逾時、log 保留天數。
        欄位下方只有「log 保留天數」附說明（記錄檔在哪裡），其他欄位不放說明文字以維持畫面精簡；
        欄位驗證錯誤仍顯示在欄位下方。
        這一頁直接修改 SettingsView 傳進來的「設定草稿」（draft），按儲存之前不會寫檔。
  誰會用到：SettingsView。
-->
<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { Settings } from "../../types/settings";
import { NUMBER_RANGES } from "../../utils/settings-form";
import NumberInput from "./NumberInput.vue";
import SettingField from "./SettingField.vue";

defineProps<{
  /** 設定草稿（尚未儲存的修改都在這裡） */
  draft: Settings;
}>();
const { t } = useI18n();

const thresholdRange = NUMBER_RANGES["general.alert_threshold_percent"];
</script>

<template>
  <div class="settings-page">
    <h2>{{ t("settings.tabs.general") }}</h2>

    <SettingField :label="t('settings.general.language')" field="general.language">
      <select v-model="draft.general.language">
        <option value="auto">{{ t("settings.general.language_auto") }}</option>
        <!-- 語言名稱用各自的語言顯示，不翻譯 -->
        <option value="zh-TW">繁體中文</option>
        <option value="en">English</option>
      </select>
    </SettingField>

    <SettingField :label="t('settings.general.autostart')">
      <input v-model="draft.general.autostart" type="checkbox" :aria-label="t('settings.general.autostart')" />
    </SettingField>

    <SettingField :label="t('settings.general.refresh_interval')" field="general.refresh_interval_minutes">
      <NumberInput
        v-model="draft.general.refresh_interval_minutes"
        field="general.refresh_interval_minutes"
        :unit="t('settings.unit.minutes')"
      />
    </SettingField>

    <SettingField :label="t('settings.general.alert_threshold')" field="general.alert_threshold_percent">
      <span class="inline">
        <input
          v-model.number="draft.general.alert_threshold_percent"
          type="range"
          :min="thresholdRange.min"
          :max="thresholdRange.max"
          :aria-label="t('settings.general.alert_threshold')"
        />
        <output class="mono">{{ draft.general.alert_threshold_percent }}%</output>
      </span>
    </SettingField>

    <SettingField :label="t('settings.general.query_timeout')" field="general.query_timeout_seconds">
      <NumberInput
        v-model="draft.general.query_timeout_seconds"
        field="general.query_timeout_seconds"
        :unit="t('settings.unit.seconds')"
      />
    </SettingField>

    <SettingField
      :label="t('settings.general.log_retention')"
      field="general.log_retention_days"
      :hint="t('settings.general.log_retention_hint')"
    >
      <NumberInput
        v-model="draft.general.log_retention_days"
        field="general.log_retention_days"
        :unit="t('settings.unit.days')"
      />
    </SettingField>
  </div>
</template>
