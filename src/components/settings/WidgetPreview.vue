<!--
  檔案：components/settings/WidgetPreview.vue
  用途：設定視窗「外觀」頁上方的即時預覽小工具（43「設定視窗」）。
        用示範資料（一個 88%、一個 45% 的額度）畫一張卡片，套上設定草稿裡的主題、色系、進度條配色、
        卡片邊框、警示外框與門檻、重置時間格式、整體不透明度；
        改了設定馬上看到效果，按儲存後才套用到真的小工具。
        卡片元件與樣式和小工具共用（ProviderCard、widgetRootStyle、data-material），看到的就是實際效果；
        玻璃主題的 Windows 材質在預覽裡用 CSS 模糊示意桌布代替。
  誰會用到：AppearancePage。
-->
<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { PREVIEW_CARD_QUOTA_IDS, previewCardProvider } from "../../services/preview-data";
import type { Settings } from "../../types/settings";
import { cardBorderStyle, resolveScheme, widgetRootStyle } from "../../utils/appearance";
import ProviderCard from "../ProviderCard.vue";

const props = defineProps<{
  /** 設定草稿 */
  draft: Settings;
  /** Windows 目前是否為深色模式（色系「跟隨 Windows」時用） */
  systemIsDark: boolean;
  /** 這台 Windows 有沒有 Acrylic（玻璃主題的樣子不同） */
  acrylic: boolean;
}>();

/** 預覽小工具的大小（px）；和小工具預設大小 360×230 一致 */
const PREVIEW_WIDTH = 360;
const PREVIEW_HEIGHT = 230;
/** 小工具四周的透明邊（base.css 的 --shell-pad 預設值） */
const PREVIEW_SHELL_PADDING = 6;
/** 倒數文字每 30 秒更新一次，和小工具相同 */
const CLOCK_TICK_MS = 30_000;

const now = ref(Date.now());
let clockTimer = 0;

const theme = computed(() => props.draft.display.theme);
const scheme = computed(() => resolveScheme(theme.value, props.draft.display.color_scheme, props.systemIsDark));
const provider = computed(() => previewCardProvider(Math.floor(now.value / 1000)));

/**
 * 預覽用的設定：沿用草稿的所有外觀設定，但示範卡片的額度一律顯示
 * （不受使用者在「服務」頁的勾選影響）。
 */
const previewSettings = computed<Settings>(() => {
  const draft = props.draft;
  return {
    ...draft,
    providers: {
      ...draft.providers,
      claude: { enabled: true, cli_path: "", visible_quotas: PREVIEW_CARD_QUOTA_IDS },
    },
  };
});

const borderMode = computed(() => cardBorderStyle(previewSettings.value, theme.value, scheme.value).mode);
const rootStyle = computed(() => widgetRootStyle(previewSettings.value, scheme.value, 1));

/** 卡片可用寬高：扣掉四周透明邊（玻璃主題套 Acrylic 時沒有透明邊，差幾 px 不影響預覽） */
const cardWidth = PREVIEW_WIDTH - PREVIEW_SHELL_PADDING * 2;
const cardHeight = PREVIEW_HEIGHT - PREVIEW_SHELL_PADDING * 2;

onMounted(() => {
  clockTimer = window.setInterval(() => (now.value = Date.now()), CLOCK_TICK_MS);
});
onBeforeUnmount(() => window.clearInterval(clockTimer));
</script>

<template>
  <div class="preview-stage" :data-stage="scheme">
    <div
      class="widget-shell preview-widget"
      :data-theme="theme"
      :data-scheme="scheme"
      data-mode="carousel"
      :data-border="borderMode"
      :data-alert-frame="draft.borders.alert_frame ? 'on' : 'off'"
      :data-material="acrylic ? 'acrylic' : 'none'"
      :style="rootStyle"
    >
      <div class="cards-area">
        <ProviderCard
          :provider="provider"
          :settings="previewSettings"
          :scheme="scheme"
          :now="now"
          :scale="1"
          :card-width="cardWidth"
          :card-height="cardHeight"
          :force-compact="false"
          :reserve-toolbar="false"
        />
      </div>
    </div>
  </div>
</template>
