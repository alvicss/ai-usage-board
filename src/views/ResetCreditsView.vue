<!--
  檔案：views/ResetCreditsView.vue
  用途：重置券小視窗（M-014，按小工具右上角的票券圖示開啟）：
        - 列出所有服務的重置券，依到期時間排序：快到期的在上面、不會過期的排最後；
          每行是「到期日期時間｜倒數｜服務（代表色小圓點＋Codex／Claude）」，滑鼠停在一行上會顯示 CLI 給的券名稱；
          CLI 只給張數、沒給到期時間的券另外寫一行「另有 N 張未提供到期時間」；
        - 外觀跟小工具同主題、同色系：根元素帶同樣的 data-theme／data-scheme／data-material／data-border，
          卡片用 .provider-card 的樣式（工程師主題也有提示字元與掃描線），細部在 styles/reset-credits.css；
        - 畫好後量出內容高度，請 Rust 調整視窗大小、放到小工具旁邊再顯示（避免閃一下或大小不對）；
        - ✕ 按鈕或 Esc 關閉；點別的地方由 Rust 自動關閉；
        - 開著時資料更新（usage-updated）清單跟著更新；倒數每 30 秒自己更新。
  誰會用到：App.vue（網址有 ?reset-credits 時）。
-->
<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { resolveLocale } from "../i18n";
import { compactResetText, resetText } from "../i18n/format";
import {
  closeResetCredits,
  getSettings,
  getSnapshot,
  getSystemInfo,
  inTauri,
  onUsageUpdated,
  revealResetCredits,
} from "../services/backend";
import { previewFrameSize } from "../services/preview-data";
import type { SystemInfo } from "../types/app";
import type { Settings } from "../types/settings";
import type { UsageSnapshot } from "../types/usage";
import { cardBorderStyle, resolveScheme, widgetRootStyle } from "../utils/appearance";
import { creditList } from "../utils/reset-credits";
import { formatAbsolute } from "../utils/time";

/** 倒數在畫面端自己更新的間隔（和小工具相同） */
const CLOCK_TICK_MS = 30_000;
/** 小視窗固定用縮放比例 1（大小由內容決定，不像小工具會跟著視窗縮放） */
const POPUP_SCALE = 1;

const { t, locale } = useI18n();

const settings = ref<Settings | null>(null);
const snapshot = ref<UsageSnapshot | null>(null);
const systemInfo = ref<SystemInfo>({ language: "en", acrylic: false });
const now = ref(Date.now());
const darkModeQuery = window.matchMedia("(prefers-color-scheme: dark)");
const systemIsDark = ref(darkModeQuery.matches);

/** 量高度用：最外層與清單 */
const shellElement = ref<HTMLElement | null>(null);
const listElement = ref<HTMLElement | null>(null);

const cleanupTasks: Array<() => void> = [];
let clockTimer = 0;

const theme = computed(() => settings.value?.display.theme ?? "minimal");

const scheme = computed(() => {
  if (settings.value === null) {
    return "dark";
  }
  return resolveScheme(settings.value.display.theme, settings.value.display.color_scheme, systemIsDark.value);
});

const borderMode = computed(() => {
  if (settings.value === null) {
    return "default";
  }
  return cardBorderStyle(settings.value, settings.value.display.theme, scheme.value).mode;
});

const rootStyle = computed(() => {
  if (settings.value === null) {
    return { "--scale": String(POPUP_SCALE) };
  }
  return widgetRootStyle(settings.value, scheme.value, POPUP_SCALE);
});

const materialAttribute = computed(() => (systemInfo.value.acrylic ? "acrylic" : "none"));

/** 清單：只列設定裡啟用的服務，依到期時間排序（utils/reset-credits.ts） */
const list = computed(() => {
  if (settings.value === null || snapshot.value === null) {
    return { items: [], undetailed: [], total: 0 };
  }
  const currentSettings = settings.value;
  const enabledProviders = snapshot.value.providers.filter(
    (provider) => currentSettings.providers[provider.id]?.enabled,
  );
  return creditList(enabledProviders, now.value);
});

const isEmpty = computed(() => list.value.items.length === 0 && list.value.undetailed.length === 0);

/** 一行的到期日期時間，例如「11/27 14:00」；不會過期時寫「不會過期」 */
function absoluteText(expiresAt: number | null): string {
  if (expiresAt === null) {
    return t("credits.never_expires");
  }
  return formatAbsolute(expiresAt);
}

/** 一行的倒數，例如「13 天 4 小時後」；工程師主題用短格式「13d4h」（和它的額度列一致）；不會過期時不顯示 */
function countdownText(expiresAt: number | null): string {
  if (expiresAt === null) {
    return "";
  }
  if (theme.value === "terminal") {
    return compactResetText(t, expiresAt, "countdown", now.value)?.text ?? "";
  }
  return resetText(t, expiresAt, "countdown", now.value)?.text ?? "";
}

/** 已經請 Rust 顯示過了沒（顯示之後，只有大小不合才再調整） */
let revealed = false;

/**
 * 量出內容完整顯示需要多高（CSS px），連同像素比例請 Rust 調整視窗大小、放到小工具旁邊並顯示。
 * 清單可能因為視窗太矮而被截短（可捲動），所以用「目前外框高度 − 清單可見高度 + 清單完整高度」算出完整高度。
 * 高度用 getBoundingClientRect 的小數值算、再多留 1px：縮放 131% 時內容常是 156.55px 這種小數，
 * 用整數量會少不到 1px，清單就冒出捲軸（2026-11-14 實測）。
 * 已經顯示、清單沒有被截短、視窗也沒有比內容高出 2px 以上時，不再調整（視窗改大小會觸發 resize，避免一直來回）。
 */
async function fitAndReveal() {
  await nextTick();
  const shell = shellElement.value;
  const listBox = listElement.value;
  if (shell === null || listBox === null) {
    return;
  }
  const shellHeight = shell.getBoundingClientRect().height;
  const listVisibleHeight = listBox.getBoundingClientRect().height;
  const neededHeight = Math.ceil(shellHeight - listVisibleHeight + listBox.scrollHeight) + 1;

  const listFits = listBox.scrollHeight <= listBox.clientHeight;
  const windowNotTooTall = window.innerHeight - neededHeight <= 2;
  if (revealed && listFits && windowNotTooTall) {
    return;
  }
  revealed = true;
  await revealResetCredits(neededHeight, window.devicePixelRatio);
}

/** 視窗大小或像素比例變了（例如被移到縮放不同的螢幕）：內容和視窗對不上就再調整一次 */
function onWindowResize() {
  void fitAndReveal();
}

function close() {
  void closeResetCredits();
}

function onKeyDown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    close();
  }
}

function onSystemSchemeChange(event: MediaQueryListEvent) {
  systemIsDark.value = event.matches;
}

/** 瀏覽器預覽（npm run dev，網址 size=寬x高）：照指定大小畫，模擬真的小視窗寬度（截圖比對用） */
function applyPreviewFrame() {
  const frame = inTauri ? null : previewFrameSize();
  if (frame === null) {
    return;
  }
  document.body.dataset.previewFrame = "on";
  document.body.style.setProperty("--preview-width", `${frame.width}px`);
  document.body.style.setProperty("--preview-height", `${frame.height}px`);
}

onMounted(async () => {
  applyPreviewFrame();
  darkModeQuery.addEventListener("change", onSystemSchemeChange);
  window.addEventListener("keydown", onKeyDown);
  window.addEventListener("resize", onWindowResize);
  cleanupTasks.push(() => darkModeQuery.removeEventListener("change", onSystemSchemeChange));
  cleanupTasks.push(() => window.removeEventListener("keydown", onKeyDown));
  cleanupTasks.push(() => window.removeEventListener("resize", onWindowResize));

  // 開著時資料更新：清單跟著換，內容高度可能改變，重新調整大小
  cleanupTasks.push(
    await onUsageUpdated((value) => {
      snapshot.value = value;
      void fitAndReveal();
    }),
  );

  const [settingsResponse, loadedSnapshot, loadedSystemInfo] = await Promise.all([
    getSettings(),
    getSnapshot(),
    getSystemInfo(),
  ]);
  systemInfo.value = loadedSystemInfo;
  settings.value = settingsResponse.settings;
  locale.value = resolveLocale(settingsResponse.settings.general.language, loadedSystemInfo.language);
  if (snapshot.value === null) {
    snapshot.value = loadedSnapshot;
  }

  clockTimer = window.setInterval(() => (now.value = Date.now()), CLOCK_TICK_MS);
  await fitAndReveal();
});

onBeforeUnmount(() => {
  for (const cleanup of cleanupTasks) {
    cleanup();
  }
  window.clearInterval(clockTimer);
});
</script>

<template>
  <main
    v-if="settings !== null && snapshot !== null"
    ref="shellElement"
    class="widget-shell credits-shell"
    :data-theme="settings.display.theme"
    :data-scheme="scheme"
    :data-border="borderMode"
    :data-material="materialAttribute"
    :style="rootStyle"
    :aria-label="t('credits.title')"
    @contextmenu.prevent
  >
    <article class="provider-card credits-card">
      <!-- 工程師主題：和卡片一樣，標題上方一行提示字元 -->
      <div v-if="theme === 'terminal'" class="terminal-prompt" aria-hidden="true">
        ~/ai-usage $ usage --reset-credits
      </div>
      <div class="card-head credits-head">
        <span class="provider-name">{{ t("credits.title") }}</span>
        <span class="credit-badge" :aria-label="t('credits.count', { count: list.total })">{{ list.total }}</span>
        <button
          type="button"
          class="tool-button credits-close"
          :title="t('credits.close')"
          :aria-label="t('credits.close')"
          @click="close"
        >
          <svg viewBox="0 0 16 16" aria-hidden="true">
            <path d="M4.5 4.5l7 7M11.5 4.5l-7 7" />
          </svg>
        </button>
      </div>

      <ul ref="listElement" class="credits-list">
        <li
          v-for="(item, index) in list.items"
          :key="`item-${index}`"
          class="credit-item"
          :title="item.title ?? undefined"
        >
          <span class="credit-when">{{ absoluteText(item.expiresAt) }}</span>
          <span class="credit-countdown">{{ countdownText(item.expiresAt) }}</span>
          <span class="credit-provider">
            <span class="provider-dot" :class="item.providerId"></span>
            {{ item.providerName }}
          </span>
        </li>
        <li v-for="entry in list.undetailed" :key="`undetailed-${entry.providerId}`" class="credit-item undetailed">
          <span class="credit-when">{{ t("credits.more_without_detail", { count: entry.count }) }}</span>
          <span class="credit-provider">
            <span class="provider-dot" :class="entry.providerId"></span>
            {{ entry.providerName }}
          </span>
        </li>
        <li v-if="isEmpty" class="credit-empty">{{ t("credits.empty") }}</li>
      </ul>
    </article>
  </main>
</template>
