<!--
  檔案：components/settings/ProvidersPage.vue
  用途：設定視窗「服務」頁（43「設定視窗」）：三家各一個區塊（ProviderSection）。
        這一頁負責：一打開就請 Rust 自動偵測三家 CLI 位置（只看檔案，不執行 CLI）、
        「瀏覽…」開檔案選擇對話框（只能選 .exe／.cmd）、Antigravity 保險絲的「我已確認，恢復查詢」。
  誰會用到：SettingsView。
-->
<script setup lang="ts">
import { onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { detectCli, resetAntigravityFuse } from "../../services/backend";
import { pickCliFile } from "../../services/dialog";
import type { RuntimeSummary } from "../../types/app";
import type { ProviderKey, Settings } from "../../types/settings";
import type { Quota, UsageSnapshot } from "../../types/usage";
import ProviderSection from "./ProviderSection.vue";

const props = defineProps<{
  /** 設定草稿 */
  draft: Settings;
  /** 最新的用量快取（額度勾選清單的來源） */
  snapshot: UsageSnapshot | null;
  /** 保險絲與看過的額度 */
  runtime: RuntimeSummary | null;
}>();
const emit = defineEmits<{ runtimeChanged: [] }>();
const { t } = useI18n();

/** 三家的固定順序、顯示名稱與 CLI 指令名稱（服務名稱與指令名稱不翻譯） */
const PROVIDERS: Array<{ id: ProviderKey; name: string; cli: string }> = [
  { id: "claude", name: "Claude", cli: "claude" },
  { id: "codex", name: "Codex", cli: "codex" },
  { id: "antigravity", name: "Antigravity", cli: "agy" },
];

/** 自動偵測結果：還沒回來是 undefined */
const detected = reactive<Record<ProviderKey, string | null | undefined>>({
  claude: undefined,
  codex: undefined,
  antigravity: undefined,
});
/** 按了「恢復查詢」之後的訊息 */
const fuseMessage = ref("");

/** 某一家最近一次查到的額度 */
function quotasOf(id: ProviderKey): Quota[] {
  if (props.snapshot === null) {
    return [];
  }
  for (const provider of props.snapshot.providers) {
    if (provider.id === id) {
      return provider.quotas;
    }
  }
  return [];
}

/** 「瀏覽…」：選到檔案就填進 CLI 位置（存不存在、副檔名對不對由 Rust 即時檢查） */
async function browse(id: ProviderKey) {
  const selected = await pickCliFile(t("settings.providers.dialog_filter"));
  if (selected !== null) {
    props.draft.providers[id].cli_path = selected;
  }
}

/** 「我已確認，恢復查詢」 */
async function resetFuse() {
  try {
    await resetAntigravityFuse();
    fuseMessage.value = t("settings.providers.fuse_resetting");
    emit("runtimeChanged");
  } catch (reason) {
    fuseMessage.value = t("settings.providers.fuse_reset_failed", { message: String(reason) });
  }
}

onMounted(async () => {
  for (const provider of PROVIDERS) {
    try {
      detected[provider.id] = await detectCli(provider.id);
    } catch {
      detected[provider.id] = null;
    }
  }
});
</script>

<template>
  <div class="settings-page">
    <h2>{{ t("settings.tabs.providers") }}</h2>
    <ProviderSection
      v-for="provider in PROVIDERS"
      :key="provider.id"
      :provider-id="provider.id"
      :name="provider.name"
      :cli-name="provider.cli"
      :provider-draft="draft.providers[provider.id]"
      :quotas="quotasOf(provider.id)"
      :detected-path="detected[provider.id]"
      :fuse="provider.id === 'antigravity' ? (runtime?.antigravity_fuse ?? null) : null"
      :fuse-message="provider.id === 'antigravity' ? fuseMessage : ''"
      @browse="browse(provider.id)"
      @reset-fuse="resetFuse"
    />
  </div>
</template>
