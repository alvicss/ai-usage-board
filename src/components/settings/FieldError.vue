<!--
  檔案：components/settings/FieldError.vue
  用途：在欄位下方用白話顯示這個欄位的錯誤（FR-9：不合法時在欄位旁說明，例如「請填 1～120 之間的整數」）。
        沒有錯誤時什麼都不顯示。錯誤文字由 SettingsView 依欄位名稱提供（form-context.ts）。
  誰會用到：SettingField，以及不用 SettingField 包起來的欄位（例如服務頁的 CLI 位置、邊框顏色）。
-->
<script setup lang="ts">
import { computed } from "vue";
import { useSettingsForm } from "./form-context";

const props = defineProps<{
  /** 欄位名稱，與 Rust 警告的 field 相同，例如 "providers.claude.cli_path" */
  field: string;
}>();

const form = useSettingsForm();
const message = computed(() => form.errorFor(props.field));
</script>

<template>
  <p v-if="message !== ''" class="field-error" role="alert">{{ message }}</p>
</template>
