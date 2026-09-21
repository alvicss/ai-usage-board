<!--
  檔案：components/ProviderCard.vue
  用途：一家服務的卡片（43「元件風格」）：
        標題列（代表色小圓點＋名稱＋方案）→ 額度 → 頁尾（「N 分鐘前更新」）。
        額度依主題用三種畫法：玻璃／簡約／霓虹是橫條（QuotaRow）、工程師是字元條（TerminalRow，
        標題列上面多一行「~/ai-usage $ usage <服務>」提示字元）、儀表是環狀表（GaugeRing）。
        查詢失敗：數字與進度條淡化變灰，標題列下方一行琥珀色原因（FR-6）；
        從未成功：卡片中央顯示原因＋怎麼解決；任一勾選的額度超過門檻：卡片外框呼吸泛紅（FR-7）。
  誰會用到：CarouselLayout、OverviewLayout、設定頁預覽 WidgetPreview。
-->
<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { compactResetText, quotaLabel, statusHelp, statusReason, updatedText } from "../i18n/format";
import type { ColorScheme, Settings } from "../types/settings";
import type { ProviderUsage } from "../types/usage";
import { gaugeRingMaxHeight, monoTextWidthEm, needsCompact, terminalRowLayout } from "../utils/layout";
import { isFailedStatus, providerHasAlert, visibleQuotas } from "../utils/quota";
import GaugeRing from "./GaugeRing.vue";
import QuotaRow from "./QuotaRow.vue";
import TerminalRow from "./TerminalRow.vue";

const props = defineProps<{
  provider: ProviderUsage;
  settings: Settings;
  scheme: ColorScheme;
  now: number;
  /** 縮放比例與卡片可用寬高（utils/layout.ts 算出） */
  scale: number;
  cardWidth: number;
  cardHeight: number;
  forceCompact: boolean;
  /** 右上角按鈕落在這張卡片的標題列時，標題列右側要留空間 */
  reserveToolbar: boolean;
}>();
const { t } = useI18n();

const theme = computed(() => props.settings.display.theme);
const quotas = computed(() => visibleQuotas(props.provider, props.settings.providers[props.provider.id]));
const failed = computed(() => isFailedStatus(props.provider));
const neverSucceeded = computed(() => props.provider.last_success_at === null);
const alert = computed(() => providerHasAlert(props.provider, props.settings));
const compact = computed(() =>
  needsCompact(props.cardHeight, quotas.value.length, props.scale, props.forceCompact, theme.value),
);

/** 有舊資料但這次查詢失敗：標題列下方的原因 */
const reason = computed(() => {
  if (!failed.value || neverSucceeded.value) {
    return "";
  }
  return statusReason(t, props.provider);
});

/** 卡片中央的訊息；null 代表正常顯示額度 */
const centerMessage = computed(() => {
  if (neverSucceeded.value) {
    // 還沒成功過：查詢中，或「原因＋怎麼解決」
    return statusHelp(t, props.provider);
  }
  if (quotas.value.length === 0) {
    return t("widget.no_quota");
  }
  return null;
});

const updated = computed(() => updatedText(t, props.provider.last_success_at, props.now));

/** 工程師主題：字元條格數與行尾要不要放重置時間，依卡片寬度、最長的標籤與重置時間決定（utils/layout.ts） */
const terminalLayout = computed(() => {
  let labelEm = 0;
  let resetEm = 0;
  for (const quota of quotas.value) {
    // 標籤前面還有一格「!」
    const labelWidth = monoTextWidthEm(`!${quotaLabel(t, quota)}`);
    if (labelWidth > labelEm) {
      labelEm = labelWidth;
    }
    const reset = compactResetText(t, quota.resets_at, props.settings.display.reset_time_format, props.now);
    if (reset !== null && !compact.value) {
      const resetWidth = monoTextWidthEm(`↻ ${reset.text}`);
      if (resetWidth > resetEm) {
        resetEm = resetWidth;
      }
    }
  }
  return terminalRowLayout(props.cardWidth, props.scale, labelEm, resetEm);
});

/** 儀表主題：環最多多高，卡片矮時環自動縮小（CSS 變數 --ring-max-h） */
const cardStyle = computed(() => {
  if (theme.value !== "gauge") {
    return undefined;
  }
  const maxHeight = gaugeRingMaxHeight(props.cardHeight, props.scale, compact.value);
  return { "--ring-max-h": `${maxHeight.toFixed(1)}px` };
});
</script>

<template>
  <article class="provider-card" :class="{ failed, alert, compact }" :style="cardStyle">
    <div class="card-top">
      <!-- 工程師主題：標題列上方一行提示字元（服務 ID 照原文，不翻譯；不使用閃爍游標）。 -->
      <div
        v-if="theme === 'terminal'"
        class="terminal-prompt"
        :class="{ 'reserve-toolbar': reserveToolbar }"
        aria-hidden="true"
      >
        ~/ai-usage $ usage {{ provider.id }}
      </div>
      <div class="card-head" :class="{ 'reserve-toolbar': reserveToolbar }">
        <span class="provider-dot" :class="provider.id"></span>
        <span class="provider-name">{{ provider.name }}</span>
        <span v-if="provider.plan" class="plan-badge">{{ provider.plan }}</span>
      </div>
      <p v-if="reason" class="status-reason" :title="reason">{{ reason }}</p>
    </div>

    <div v-if="centerMessage !== null" class="card-message" :class="{ problem: failed }">
      {{ centerMessage }}
    </div>
    <div v-else-if="theme === 'gauge'" class="gauge-list">
      <GaugeRing
        v-for="quota in quotas"
        :key="quota.id"
        :quota="quota"
        :colors="settings.colors"
        :scheme="scheme"
        :threshold="settings.general.alert_threshold_percent"
        :reset-format="settings.display.reset_time_format"
        :now="now"
        :compact="compact"
        :failed="failed"
      />
    </div>
    <div v-else-if="theme === 'terminal'" class="quota-list terminal-list">
      <TerminalRow
        v-for="quota in quotas"
        :key="quota.id"
        :quota="quota"
        :colors="settings.colors"
        :scheme="scheme"
        :threshold="settings.general.alert_threshold_percent"
        :reset-format="settings.display.reset_time_format"
        :now="now"
        :compact="compact || !terminalLayout.showReset"
        :failed="failed"
        :cells="terminalLayout.cells"
      />
    </div>
    <div v-else class="quota-list">
      <QuotaRow
        v-for="quota in quotas"
        :key="quota.id"
        :quota="quota"
        :colors="settings.colors"
        :scheme="scheme"
        :threshold="settings.general.alert_threshold_percent"
        :reset-format="settings.display.reset_time_format"
        :now="now"
        :compact="compact"
        :failed="failed"
      />
    </div>

    <footer class="card-foot">
      <span class="updated-text">{{ updated }}</span>
    </footer>
  </article>
</template>
