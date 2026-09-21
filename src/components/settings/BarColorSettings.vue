<!--
  檔案：components/settings/BarColorSettings.vue
  用途：外觀頁的「進度條配色」（FR-13）：0%、中間色、100% 三個顏色選擇器＋中間色位置滑桿（1～99%）＋還原預設。
        叫「中間色」而不是「橘色」：使用者可以把它改成任何顏色。
        五種主題共用這組配色；改了之後上方預覽馬上跟著變。
  誰會用到：AppearancePage。
-->
<script setup lang="ts">
import { useI18n } from "vue-i18n";
import type { Settings } from "../../types/settings";
import { DEFAULT_COLORS, NUMBER_RANGES } from "../../utils/settings-form";
import FieldError from "./FieldError.vue";
import SettingField from "./SettingField.vue";

const props = defineProps<{
  /** 設定草稿 */
  draft: Settings;
}>();
const { t } = useI18n();

const midRange = NUMBER_RANGES["colors.mid_position"];

/** 還原預設：綠 #01B468 → 60% 橘 #EA7500 → 紅 #FF0000 */
function restoreDefaults() {
  props.draft.colors.low = DEFAULT_COLORS.low;
  props.draft.colors.mid = DEFAULT_COLORS.mid;
  props.draft.colors.high = DEFAULT_COLORS.high;
  props.draft.colors.mid_position = DEFAULT_COLORS.mid_position;
}
</script>

<template>
  <SettingField :label="t('settings.appearance.bar_colors')">
    <span class="inline">
      <input
        v-model="draft.colors.low"
        class="swatch"
        type="color"
        :title="t('settings.appearance.color_low')"
        :aria-label="t('settings.appearance.color_low')"
      />
      <input
        v-model="draft.colors.mid"
        class="swatch"
        type="color"
        :title="t('settings.appearance.color_mid')"
        :aria-label="t('settings.appearance.color_mid')"
      />
      <input
        v-model="draft.colors.high"
        class="swatch"
        type="color"
        :title="t('settings.appearance.color_high')"
        :aria-label="t('settings.appearance.color_high')"
      />
      <!-- 三色的漸變示意：和進度條用同一組顏色與中間色位置 -->
      <span
        class="gradient-sample"
        :style="{
          background: `linear-gradient(90deg, ${draft.colors.low} 0%, ${draft.colors.mid} ${draft.colors.mid_position}%, ${draft.colors.high} 100%)`,
        }"
        aria-hidden="true"
      ></span>
      <button type="button" class="button" @click="restoreDefaults">
        {{ t("settings.appearance.reset_default") }}
      </button>
    </span>
    <FieldError field="colors.low" />
    <FieldError field="colors.mid" />
    <FieldError field="colors.high" />
  </SettingField>

  <SettingField :label="t('settings.appearance.mid_position')" field="colors.mid_position">
    <span class="inline">
      <input
        v-model.number="draft.colors.mid_position"
        type="range"
        :min="midRange.min"
        :max="midRange.max"
        :aria-label="t('settings.appearance.mid_position')"
      />
      <output class="mono">{{ draft.colors.mid_position }}%</output>
    </span>
  </SettingField>
</template>
