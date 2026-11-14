<!--
  檔案：components/ResetCreditRow.vue
  用途：卡片額度下方的重置券那一行（M-014）：「重置券 ② 最快到期 13 天 4 小時後」。
        - 張數用圓形數字徽章（2 張以上只顯示張數＋最快到期的那張，完整清單在重置券小視窗）；
        - 到期時間沿用「重置時間格式」設定（倒數／絕對時間／兩者）；工程師與儀表主題用短格式，和它們的額度列一致；
        - 顏色、字型全部用主題的 CSS 變數（--w-ink、--w-muted、--w-font…），五種主題自動套用原本的風格
          （各主題的細部調整在 styles/reset-credits.css）。
  誰會用到：ProviderCard。
-->
<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { creditExpiryText } from "../i18n/format";
import type { ResetTimeFormat, ThemeId } from "../types/settings";
import type { CreditSummary } from "../utils/reset-credits";

const props = defineProps<{
  summary: CreditSummary;
  resetFormat: ResetTimeFormat;
  /** 現在時間（毫秒） */
  now: number;
  theme: ThemeId;
}>();
const { t } = useI18n();

/** 工程師、儀表主題的時間用短格式（2h13m、3d4h） */
const shortForm = computed(() => props.theme === "terminal" || props.theme === "gauge");

const expiryText = computed(() =>
  creditExpiryText(t, props.summary, props.resetFormat, props.now, shortForm.value),
);

/** 徽章給螢幕報讀用的完整說法，例如「2 張」 */
const countLabel = computed(() => t("credits.count", { count: props.summary.count }));
</script>

<template>
  <div class="credit-row" :title="`${t('credits.label')} ${countLabel} · ${expiryText}`">
    <span class="credit-label">{{ t("credits.label") }}</span>
    <span class="credit-badge" :aria-label="countLabel">{{ summary.count }}</span>
    <span class="credit-expiry">{{ expiryText }}</span>
  </div>
</template>
