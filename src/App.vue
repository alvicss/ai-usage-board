<!--
  檔案：App.vue
  用途：依目前 Tauri 視窗載入小工具、獨立設定畫面，或重置券小視窗（網址 ?reset-credits，M-014）。
  誰會用到：src/main.ts。
-->
<script setup lang="ts">
import ResetCreditsView from "./views/ResetCreditsView.vue";
import SettingsView from "./views/SettingsView.vue";
import WidgetView from "./views/WidgetView.vue";

const parameters = new URLSearchParams(location.search);
const isSettingsWindow = parameters.has("settings");
const isResetCreditsWindow = parameters.has("reset-credits");
</script>

<template>
  <SettingsView v-if="isSettingsWindow" />
  <ResetCreditsView v-else-if="isResetCreditsWindow" />
  <WidgetView v-else />
</template>
