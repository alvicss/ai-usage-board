<!--
  檔案：components/settings/BorderSettings.vue
  用途：外觀頁的「卡片邊框」與「警示外框」（FR-18、43「邊框自訂」）：
        - 卡片邊框：只顯示「目前主題＋目前色系」那一組（標題註明是哪一組），三選一：主題預設／自訂顏色／無邊框，
          有還原預設；切到別的主題或色系時，改的就是那一組（9 組各自記住）。
        - 警示外框：開關＋顏色選擇器＋還原預設；警示顏色同時用在該行底色與輪播圓點。
  誰會用到：AppearancePage。
-->
<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { ColorScheme, Settings, ThemeId } from "../../types/settings";
import { isHexColor, type BorderMode } from "../../utils/appearance";
import {
  DEFAULT_ALERT_COLOR,
  DEFAULT_ALERT_FRAME,
  DEFAULT_CUSTOM_BORDER_COLOR,
  borderFieldKey,
  borderModeOf,
  borderValueFor,
} from "../../utils/settings-form";
import FieldError from "./FieldError.vue";
import SettingField from "./SettingField.vue";

const props = defineProps<{
  /** 設定草稿 */
  draft: Settings;
  /** 目前主題 */
  theme: ThemeId;
  /** 目前主題實際的亮暗（已把「跟隨 Windows」換算好；霓虹固定暗色） */
  scheme: ColorScheme;
}>();
const { t } = useI18n();

/** 三個選項（順序與 43 相同） */
const BORDER_MODES: BorderMode[] = ["default", "custom", "none"];

/** 目前在改 [borders.card] 的哪一個欄位，例如 minimal_dark */
const cardKey = computed(() => borderFieldKey(props.theme, props.scheme));
const cardValue = computed(() => props.draft.borders.card[cardKey.value]);
const cardMode = computed(() => borderModeOf(cardValue.value));
/** 標題註明目前是哪個主題、哪個色系 */
const targetText = computed(() =>
  t("settings.appearance.card_border_target", {
    theme: t(`theme.${props.theme}`),
    scheme: t(`settings.appearance.scheme_${props.scheme}`),
  }),
);

/** 自訂顏色選擇器的顏色：這一組已經是自訂色就用它，否則用一個起始色 */
const customColor = ref(DEFAULT_CUSTOM_BORDER_COLOR);
watch(
  cardValue,
  (value) => {
    if (isHexColor(value)) {
      customColor.value = value;
    }
  },
  { immediate: true },
);

/** 選了主題預設／自訂顏色／無邊框 */
function chooseMode(mode: BorderMode) {
  props.draft.borders.card[cardKey.value] = borderValueFor(mode, customColor.value);
}

/** 調了邊框顏色：直接切成「自訂顏色」 */
function chooseCustomColor(event: Event) {
  const color = (event.target as HTMLInputElement).value;
  customColor.value = color;
  props.draft.borders.card[cardKey.value] = color;
}

function restoreCardBorder() {
  props.draft.borders.card[cardKey.value] = "default";
}

function restoreAlertFrame() {
  props.draft.borders.alert_frame = DEFAULT_ALERT_FRAME;
  props.draft.borders.alert_color = DEFAULT_ALERT_COLOR;
}
</script>

<template>
  <SettingField :label="t('settings.appearance.card_border')">
    <div class="stack">
      <p class="border-target">{{ targetText }}</p>
      <div class="inline" role="radiogroup" :aria-label="t('settings.appearance.card_border')">
        <label v-for="mode in BORDER_MODES" :key="mode" class="check">
          <input
            type="radio"
            name="card-border-mode"
            :value="mode"
            :checked="cardMode === mode"
            @change="chooseMode(mode)"
          />
          <span>{{ t(`settings.appearance.border_${mode}`) }}</span>
        </label>
      </div>
      <div class="inline">
        <input
          class="swatch"
          type="color"
          :value="customColor"
          :title="t('settings.appearance.border_color')"
          :aria-label="t('settings.appearance.border_color')"
          @input="chooseCustomColor"
        />
        <button type="button" class="button" @click="restoreCardBorder">
          {{ t("settings.appearance.reset_default") }}
        </button>
      </div>
      <FieldError :field="`borders.card.${cardKey}`" />
    </div>
  </SettingField>

  <SettingField :label="t('settings.appearance.alert_frame')">
    <div class="stack">
      <input v-model="draft.borders.alert_frame" type="checkbox" :aria-label="t('settings.appearance.alert_frame')" />
      <div class="inline">
        <input
          v-model="draft.borders.alert_color"
          class="swatch"
          type="color"
          :title="t('settings.appearance.alert_color')"
          :aria-label="t('settings.appearance.alert_color')"
        />
        <button type="button" class="button" @click="restoreAlertFrame">
          {{ t("settings.appearance.reset_default") }}
        </button>
      </div>
      <FieldError field="borders.alert_color" />
    </div>
  </SettingField>
</template>
