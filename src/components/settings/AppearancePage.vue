<!--
  檔案：components/settings/AppearancePage.vue
  用途：設定視窗「外觀」頁（43「設定視窗」）：上方即時預覽；主題（五張縮圖，霓虹標「僅暗色」）、色系、顯示方式、
        輪播秒數與動畫、重置時間格式；整體不透明度（滑桿 30～100%，FR-16）；
        進度條配色（BarColorSettings）；卡片邊框與警示外框（BorderSettings，FR-18）。
        這一頁不放說明文字，以維持畫面精簡；只保留功能性的標示（例如「僅暗色」）與欄位錯誤。
        這一頁直接修改 SettingsView 傳進來的「設定草稿」（draft），按儲存之前不會寫檔。
  誰會用到：SettingsView。
-->
<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { Settings, ThemeId } from "../../types/settings";
import { resolveScheme } from "../../utils/appearance";
import { NUMBER_RANGES } from "../../utils/settings-form";
import BarColorSettings from "./BarColorSettings.vue";
import BorderSettings from "./BorderSettings.vue";
import NumberInput from "./NumberInput.vue";
import SettingField from "./SettingField.vue";
import WidgetPreview from "./WidgetPreview.vue";

const props = defineProps<{
  /** 設定草稿 */
  draft: Settings;
  /** Windows 目前是否為深色模式（色系「跟隨 Windows」時用） */
  systemIsDark: boolean;
  /** 這台 Windows 有沒有 Acrylic（預覽的玻璃主題要畫成哪個樣子） */
  acrylic: boolean;
}>();
const { t } = useI18n();

/** 五種主題（順序與 43「五種主題」相同） */
const THEMES: ThemeId[] = ["glass", "minimal", "terminal", "gauge", "neon"];

/** 整體不透明度的範圍（30～100%，與 Rust validate.rs 相同） */
const opacityRange = NUMBER_RANGES["display.opacity_percent"];

/** 目前主題實際用亮色還是暗色（邊框設定要知道改的是哪一組） */
const scheme = computed(() =>
  resolveScheme(props.draft.display.theme, props.draft.display.color_scheme, props.systemIsDark),
);

function chooseTheme(theme: ThemeId) {
  props.draft.display.theme = theme;
}
</script>

<template>
  <div class="settings-page">
    <h2>{{ t("settings.tabs.appearance") }}</h2>

    <section class="preview-block" :aria-label="t('settings.appearance.preview')">
      <WidgetPreview :draft="draft" :system-is-dark="systemIsDark" :acrylic="acrylic" />
    </section>

    <SettingField :label="t('settings.appearance.theme')" field="display.theme">
      <div class="theme-thumbs">
        <button
          v-for="theme in THEMES"
          :key="theme"
          type="button"
          class="theme-thumb"
          :data-thumb="theme"
          :aria-pressed="draft.display.theme === theme"
          @click="chooseTheme(theme)"
        >
          <span>{{ t(`theme.${theme}`) }}</span>
          <small v-if="theme === 'neon'">{{ t("settings.appearance.dark_only") }}</small>
        </button>
      </div>
    </SettingField>

    <SettingField :label="t('settings.appearance.color_scheme')" field="display.color_scheme">
      <select v-model="draft.display.color_scheme">
        <option value="auto">{{ t("settings.appearance.scheme_auto") }}</option>
        <option value="light">{{ t("settings.appearance.scheme_light") }}</option>
        <option value="dark">{{ t("settings.appearance.scheme_dark") }}</option>
      </select>
    </SettingField>

    <SettingField :label="t('settings.appearance.mode')" field="display.mode">
      <select v-model="draft.display.mode">
        <option value="carousel">{{ t("settings.appearance.mode_carousel") }}</option>
        <option value="overview">{{ t("settings.appearance.mode_overview") }}</option>
      </select>
    </SettingField>

    <SettingField :label="t('settings.appearance.carousel_seconds')" field="display.carousel_seconds">
      <NumberInput
        v-model="draft.display.carousel_seconds"
        field="display.carousel_seconds"
        :unit="t('settings.unit.seconds')"
      />
    </SettingField>

    <SettingField :label="t('settings.appearance.animation')" field="display.carousel_animation">
      <select v-model="draft.display.carousel_animation">
        <option value="slide">{{ t("settings.appearance.animation_slide") }}</option>
        <option value="flip">{{ t("settings.appearance.animation_flip") }}</option>
      </select>
    </SettingField>

    <SettingField :label="t('settings.appearance.reset_format')" field="display.reset_time_format">
      <select v-model="draft.display.reset_time_format">
        <option value="countdown">{{ t("settings.appearance.reset_countdown") }}</option>
        <option value="absolute">{{ t("settings.appearance.reset_absolute") }}</option>
        <option value="both">{{ t("settings.appearance.reset_both") }}</option>
      </select>
    </SettingField>

    <SettingField :label="t('settings.appearance.opacity')" field="display.opacity_percent">
      <span class="inline">
        <input
          v-model.number="draft.display.opacity_percent"
          type="range"
          :min="opacityRange.min"
          :max="opacityRange.max"
          :aria-label="t('settings.appearance.opacity')"
        />
        <output class="mono">{{ draft.display.opacity_percent }}%</output>
      </span>
    </SettingField>

    <BarColorSettings :draft="draft" />
    <BorderSettings :draft="draft" :theme="draft.display.theme" :scheme="scheme" />
  </div>
</template>
