<!--
  檔案：App.vue
  用途：依目前 Tauri 視窗載入小工具或獨立設定畫面。
  誰會用到：src/main.ts。
-->
<script setup lang="ts">
import SettingsView from "./views/SettingsView.vue";
import WidgetView from "./views/WidgetView.vue";
const isSettingsWindow = new URLSearchParams(location.search).has("settings");
</script>

<template>
  <SettingsView v-if="isSettingsWindow" />
  <WidgetView v-else />
</template>
