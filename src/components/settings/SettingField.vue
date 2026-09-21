<!--
  檔案：components/settings/SettingField.vue
  用途：設定視窗的一列欄位：左邊標籤、右邊輸入控制項（由使用者放進 slot），
        下方可選的說明文字與錯誤文字（43「設定視窗」：欄位錯誤時在欄位下方用白話說明）。
        整列標成一個「群組」並以標籤命名，螢幕報讀軟體才知道輸入框屬於哪個設定。
  誰會用到：設定視窗各頁（GeneralPage、AppearancePage、BarColorSettings…）。
-->
<script setup lang="ts">
import { useId } from "vue";
import FieldError from "./FieldError.vue";

defineProps<{
  /** 左邊的標籤文字 */
  label: string;
  /** 欄位名稱（例如 "general.refresh_interval_minutes"），有給才會顯示這個欄位的錯誤 */
  field?: string;
  /** 控制項下方的說明（可省略） */
  hint?: string;
}>();

/** 標籤的唯一 id（群組用它當名稱） */
const labelId = useId();
</script>

<template>
  <div class="field" role="group" :aria-labelledby="labelId">
    <span :id="labelId" class="field-label">{{ label }}</span>
    <div class="field-control">
      <slot />
    </div>
    <p v-if="hint" class="field-hint">{{ hint }}</p>
    <FieldError v-if="field" class="field-message" :field="field" />
  </div>
</template>
