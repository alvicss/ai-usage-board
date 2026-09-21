<!--
  檔案：components/settings/AboutPage.vue
  用途：設定視窗「關於」頁（FR-15）：程式名稱、兩碼版本（例如 v1.0）、作者「半桶水的AI筆記」、GitHub 連結、MIT 授權。
        兩個連結用藍色（樣式 .link-button，在 styles/settings.css），一看就知道可以點。
        點作者或 GitHub 時請 Rust 用 Windows 預設瀏覽器開啟；程式本身不開網頁、不連網（22 資安規則 2），
        網址寫在 Rust 端（about.rs），畫面不能指定。
  誰會用到：SettingsView。
-->
<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { openHomepage, openRepository } from "../../services/backend";
import type { AppInfo } from "../../types/app";

defineProps<{
  /** 程式名稱與版本（載入中時為 null） */
  appInfo: AppInfo | null;
}>();
const { t } = useI18n();

const openError = ref("");

/** 請 Rust 開瀏覽器；失敗時在頁面上顯示原因 */
async function openInBrowser(open: () => Promise<void>) {
  openError.value = "";
  try {
    await open();
  } catch (reason) {
    openError.value = t("settings.about.open_failed", { message: String(reason) });
  }
}

function openAuthorSite() {
  void openInBrowser(openHomepage);
}

function openRepositoryPage() {
  void openInBrowser(openRepository);
}
</script>

<template>
  <div class="settings-page about-page">
    <h2>{{ t("settings.tabs.about") }}</h2>
    <p class="app-name">{{ appInfo?.name ?? "AI Usage Board" }}</p>
    <p class="mono">{{ t("settings.about.version") }} {{ appInfo?.version ?? "" }}</p>
    <p>
      {{ t("settings.about.author") }}
      <button type="button" class="link-button" :title="t('settings.about.author_hint')" @click="openAuthorSite">
        {{ t("settings.about.author_name") }}
      </button>
    </p>
    <p>
      {{ t("settings.about.repository") }}
      <button type="button" class="link-button" :title="t('settings.about.repository_hint')" @click="openRepositoryPage">
        {{ t("settings.about.repository_name") }}
      </button>
    </p>
    <p v-if="openError" class="field-error">{{ openError }}</p>
    <p>{{ t("settings.about.license") }}</p>
  </div>
</template>
