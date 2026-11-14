<!--
  檔案：components/ToolbarButtons.vue
  用途：小工具右上角的四顆按鈕：⟳ 立即更新（查詢中持續轉圈、不能再按）、⇄ 切換輪播／並排、
        票券 開關重置券小視窗（M-014，一直顯示）、⚙ 開啟設定。
        圖示用 SVG（Windows 縮放 100%～200% 都清晰），一律 16×16 格、1.6 線寬、圓頭線條（樣式在 base.css 的 .tool-button svg）。
  誰會用到：WidgetView。
-->
<script setup lang="ts">
import { useI18n } from "vue-i18n";

defineProps<{ refreshing: boolean }>();
const emit = defineEmits<{ refresh: []; switchMode: []; resetCredits: []; settings: [] }>();
const { t } = useI18n();
</script>

<template>
  <div class="toolbar">
    <button
      type="button"
      class="tool-button"
      :class="{ spinning: refreshing }"
      :disabled="refreshing"
      :title="t('app.refresh')"
      :aria-label="t('app.refresh')"
      @click="emit('refresh')"
    >
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <path d="M13.5 8a5.5 5.5 0 1 1-1.6-3.9" />
        <path d="M12.4 1.5v2.9h-2.9" />
      </svg>
    </button>
    <button
      type="button"
      class="tool-button"
      :title="t('app.switch_mode')"
      :aria-label="t('app.switch_mode')"
      @click="emit('switchMode')"
    >
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <path d="M2.5 5.5h10M10 3l2.5 2.5L10 8" />
        <path d="M13.5 10.5h-10M6 8l-2.5 2.5L6 13" />
      </svg>
    </button>
    <button
      type="button"
      class="tool-button"
      :title="t('app.reset_credits')"
      :aria-label="t('app.reset_credits')"
      @click="emit('resetCredits')"
    >
      <!-- 票券：左右兩側各有一個半圓缺口，右邊一條撕線（三小段） -->
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <path d="M2.5 4.5h11v2.1a1.4 1.4 0 0 0 0 2.8v2.1h-11V9.4a1.4 1.4 0 0 0 0-2.8z" />
        <path d="M10 5.6v.8M10 7.6v.8M10 9.6v.8" />
      </svg>
    </button>
    <button
      type="button"
      class="tool-button"
      :title="t('app.settings')"
      :aria-label="t('app.settings')"
      @click="emit('settings')"
    >
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <circle cx="8" cy="8" r="5.6" stroke-width="2.4" stroke-dasharray="2.2 2.2" />
        <circle cx="8" cy="8" r="4.3" />
        <circle cx="8" cy="8" r="1.7" />
      </svg>
    </button>
  </div>
</template>
