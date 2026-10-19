<!--
  檔案：components/settings/ProviderSection.vue
  用途：設定視窗「服務」頁裡的一家服務（43「設定視窗」、FR-11、FR-12）：
        - 啟用開關；
        - CLI 位置：顯示自動偵測結果；可手打或「瀏覽…」選 .exe／.cmd；「改回自動偵測」；檔案不存在時欄位下方即時提示；
        - 額度勾選清單：來自最新一次查到的額度（新出現的額度預設不勾，不另外標示）；
        - Antigravity 保險絲跳脫時，顯示原因與「我已確認，恢復查詢」按鈕（事件交給上一層處理）。
        這個元件直接修改傳進來的「這一家的設定草稿」，按儲存之前不會寫檔。
  誰會用到：ProvidersPage。
-->
<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { quotaLabel } from "../../i18n/format";
import type { AntigravityFuse } from "../../types/app";
import type { ProviderKey, ProviderSettings } from "../../types/settings";
import type { Quota } from "../../types/usage";
import { formatAbsolute } from "../../utils/time";
import FieldError from "./FieldError.vue";

const props = defineProps<{
  providerId: ProviderKey;
  /** 顯示名稱（Claude／Codex／Antigravity，不翻譯） */
  name: string;
  /** CLI 指令名稱（claude／codex／agy，提示文字用） */
  cliName: string;
  /** 這一家的設定草稿 */
  providerDraft: ProviderSettings;
  /** 最近一次查到的額度 */
  quotas: Quota[];
  /** 自動偵測結果：undefined＝偵測中、null＝找不到、字串＝找到的路徑 */
  detectedPath: string | null | undefined;
  /** 保險絲狀態（只有 Antigravity 會傳；跳脫時才顯示提示） */
  fuse?: AntigravityFuse | null;
  /** 保險絲恢復後的訊息（例如「已恢復，正在重新查詢」或失敗原因） */
  fuseMessage?: string;
}>();
const emit = defineEmits<{ browse: []; resetFuse: [] }>();
const { t } = useI18n();

/** 自動偵測那一行的文字 */
const detectionText = computed(() => {
  if (props.detectedPath === undefined) {
    return t("settings.providers.detecting");
  }
  if (props.detectedPath === null) {
    return t("settings.providers.not_detected", { cli: props.cliName });
  }
  return t("settings.providers.detected", { path: props.detectedPath });
});

/** 保險絲跳脫的原因文字 */
const fuseText = computed(() => {
  const fuse = props.fuse;
  if (fuse === undefined || fuse === null || !fuse.tripped) {
    return "";
  }
  const time = fuse.tripped_at === null ? "" : formatAbsolute(fuse.tripped_at);
  if (fuse.num_turns === null) {
    return t("settings.providers.fuse_unknown", { time });
  }
  return t("settings.providers.fuse_turns", { turns: fuse.num_turns, time });
});

function useAutoDetect() {
  props.providerDraft.cli_path = "";
}
</script>

<template>
  <section class="service">
    <div class="service-head">
      <span class="inline">
        <span class="service-dot" :class="providerId"></span>
        <strong>{{ name }}</strong>
      </span>
      <label class="check">
        <input v-model="providerDraft.enabled" type="checkbox" />
        <span>{{ t("settings.providers.enabled") }}</span>
      </label>
    </div>

    <div class="service-body" :class="{ disabled: !providerDraft.enabled }">
      <div class="field">
        <span class="field-label">{{ t("settings.providers.cli_path") }}</span>
        <div class="field-control cli-control">
          <input
            v-model.trim="providerDraft.cli_path"
            class="path-input"
            type="text"
            spellcheck="false"
            :placeholder="t('settings.providers.cli_placeholder')"
          />
          <button type="button" class="button" @click="emit('browse')">{{ t("settings.providers.browse") }}</button>
          <button v-if="providerDraft.cli_path !== ''" type="button" class="button" @click="useAutoDetect">
            {{ t("settings.providers.use_auto") }}
          </button>
        </div>
        <p class="field-hint mono path-text">{{ detectionText }}</p>
        <p v-if="providerDraft.cli_path !== ''" class="field-hint">{{ t("settings.providers.custom_in_use") }}</p>
        <FieldError class="field-message" :field="`providers.${providerId}.cli_path`" />
      </div>

      <div class="field">
        <span class="field-label">{{ t("settings.providers.quotas") }}</span>
        <div class="field-control">
          <div v-if="quotas.length > 0" class="quota-checks">
            <label v-for="quota in quotas" :key="quota.id" class="check">
              <input v-model="providerDraft.visible_quotas" type="checkbox" :value="quota.id" />
              <span>{{ quotaLabel(t, quota) }}</span>
            </label>
          </div>
          <p v-else class="field-hint">{{ t("settings.providers.no_quotas") }}</p>
        </div>
      </div>
    </div>

    <div v-if="fuseText !== '' || fuseMessage" class="fuse-box" role="alert">
      <template v-if="fuseText !== ''">
        <p>{{ fuseText }}</p>
        <p>{{ t("settings.providers.fuse_advice") }}</p>
        <button type="button" class="button" @click="emit('resetFuse')">{{ t("settings.providers.fuse_reset") }}</button>
      </template>
      <p v-if="fuseMessage">{{ fuseMessage }}</p>
    </div>
  </section>
</template>
