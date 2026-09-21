<!--
  檔案：components/settings/NumberInput.vue
  用途：只收整數的輸入框（例如更新間隔、查詢逾時）。
        - 使用者打的文字是整數：更新設定值（範圍對不對交給 Rust 的即時檢查，錯了會顯示在欄位下方）；
        - 不是整數（空白、小數、夾雜文字）：不改設定值，改回報「這欄目前看不懂」，欄位下方提示合法範圍，
          儲存按鈕也會擋下來。
        提示的合法範圍來自 utils/settings-form.ts 的 NUMBER_RANGES（由 SettingsView 依目前語言組成文字）。
  誰會用到：GeneralPage、AppearancePage。
-->
<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from "vue";
import { parseInteger, type NumberField } from "../../utils/settings-form";
import { useSettingsForm } from "./form-context";

const props = defineProps<{
  modelValue: number;
  /** 欄位名稱：決定提示的合法範圍，也用來顯示錯誤 */
  field: NumberField;
  /** 輸入框後面的單位（例如「分鐘」） */
  unit?: string;
}>();
const emit = defineEmits<{ "update:modelValue": [value: number] }>();

const form = useSettingsForm();
/** 輸入框目前的文字（可能暫時不是數字） */
const text = ref(String(props.modelValue));

/** 使用者每打一個字：能轉成整數就更新設定值，不能就回報錯誤 */
function onInput(event: Event) {
  const input = event.target as HTMLInputElement;
  text.value = input.value;
  const parsed = parseInteger(input.value);
  if (parsed === null) {
    form.setLocalError(props.field, true);
    return;
  }
  form.setLocalError(props.field, false);
  emit("update:modelValue", parsed);
}

// 設定值從外面被改掉（例如儲存後 Rust 回傳的值）時，輸入框文字跟著更新
watch(
  () => props.modelValue,
  (value) => {
    if (parseInteger(text.value) !== value) {
      text.value = String(value);
      form.setLocalError(props.field, false);
    }
  },
);

// 換頁時元件被移除：清掉這一欄的暫時錯誤，避免擋住儲存
onBeforeUnmount(() => {
  if (parseInteger(text.value) === null) {
    text.value = String(props.modelValue);
  }
  form.setLocalError(props.field, false);
});
</script>

<template>
  <span class="inline">
    <input
      class="number-input"
      type="text"
      inputmode="numeric"
      :value="text"
      @input="onInput"
    />
    <span v-if="unit" class="unit">{{ unit }}</span>
  </span>
</template>
