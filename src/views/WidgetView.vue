<!--
  檔案：views/WidgetView.vue
  用途：小工具主畫面的組裝者：
        - 啟動時向 Rust 取設定、快取與電腦環境（語言「自動」的結果、有沒有 Acrylic），
          之後聽 usage-updated／refresh-state／settings-changed 事件即時更新；
        - 算出主題、實際色系（跟隨 Windows 時即時切換）、縮放比例與排版，套在根元素的 data-* 與 CSS 變數上
          （data-material：這台 Windows 有沒有 Acrylic，玻璃主題依此決定樣子）；
        - 語言：設定手動選的優先，「自動」用 Rust 依 Windows 顯示語言與地區判斷的結果（FR-17），換語言時右鍵選單跟著重建；
        - 右上角 ⟳ ⇄ ⚙ 按鈕、原生右鍵選單、拖曳視窗；倒數與「幾分鐘前」每 30 秒在畫面端自己更新，不重查；
        - 拖曳或縮放停下來後，請 Rust 記住目前顯示方式的位置大小（FR-14）；
        - 重複開啟程式時（單一執行個體），收到 widget-flash 事件就閃一下外框提示位置；
        - settings.toml 整份格式錯誤時，頂端顯示白話警告（FR-9）。
  誰會用到：App.vue。
-->
<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import CarouselLayout from "../components/CarouselLayout.vue";
import OverviewLayout from "../components/OverviewLayout.vue";
import ToolbarButtons from "../components/ToolbarButtons.vue";
import { resolveLocale } from "../i18n";
import {
  getSettings,
  getSnapshot,
  getSystemInfo,
  inTauri,
  onRefreshState,
  onSettingsChanged,
  onUsageUpdated,
  onWidgetFlash,
  openSettings,
  quitApp,
  refreshNow,
  saveWindowBounds,
  setDisplayMode,
} from "../services/backend";
import { previewFrameSize, previewWantsStage } from "../services/preview-data";
import {
  buildContextMenu,
  onWindowMovedOrResized,
  popupContextMenu,
  startWindowDrag,
} from "../services/window";
import type { SystemInfo } from "../types/app";
import type { Settings, SettingsWarning } from "../types/settings";
import type { UsageSnapshot } from "../types/usage";
import { cardBorderStyle, resolveScheme, widgetRootStyle } from "../utils/appearance";
import { computeLayout } from "../utils/layout";

/** 倒數與「幾分鐘前」在畫面端自己更新的間隔 */
const CLOCK_TICK_MS = 30_000;
/** 視窗移動／縮放停止多久後才記住位置（避免拖曳中一直寫檔） */
const BOUNDS_SAVE_DELAY_MS = 400;
/** 重複開啟時外框閃爍的時間（與 Rust widget_window.rs 的 FLASH_DURATION 相同） */
const FLASH_DURATION_MS = 1800;

const { t, locale } = useI18n();

const settings = ref<Settings | null>(null);
const warnings = ref<SettingsWarning[]>([]);
const snapshot = ref<UsageSnapshot | null>(null);
const refreshing = ref(false);
const now = ref(Date.now());
const windowWidth = ref(window.innerWidth);
const windowHeight = ref(window.innerHeight);
/** 電腦環境：語言「自動」的結果、有沒有 Acrylic（啟動時向 Rust 取一次） */
const systemInfo = ref<SystemInfo>({ language: "en", acrylic: false });
/** 正在閃爍提示位置（重複開啟程式時） */
const flashing = ref(false);

// Windows 的淺色／深色模式（跟隨 Windows 時用）
const darkModeQuery = window.matchMedia("(prefers-color-scheme: dark)");
const systemIsDark = ref(darkModeQuery.matches);

const cleanupTasks: Array<() => void> = [];
let clockTimer = 0;
let boundsTimer = 0;
let flashTimer = 0;
/** 右鍵選單目前是用哪個語言建立的（語言沒變就不重建） */
let menuLocale = "";
/** 滑鼠是否停在小工具上（停在上面時暫停輪播，FR-4） */
const hovering = ref(false);

/** 要顯示的服務：設定裡啟用的才顯示（固定順序 claude、codex、antigravity） */
const providers = computed(() => {
  if (settings.value === null || snapshot.value === null) {
    return [];
  }
  const currentSettings = settings.value;
  return snapshot.value.providers.filter((provider) => currentSettings.providers[provider.id]?.enabled);
});

const scheme = computed(() => {
  if (settings.value === null) {
    return "dark";
  }
  return resolveScheme(settings.value.display.theme, settings.value.display.color_scheme, systemIsDark.value);
});

const layout = computed(() => {
  const mode = settings.value?.display.mode ?? "carousel";
  const cardCount = mode === "overview" ? providers.value.length : 1;
  return computeLayout(windowWidth.value, windowHeight.value, mode, cardCount);
});

const borderStyle = computed(() => {
  if (settings.value === null) {
    return { mode: "default", variables: {} as Record<string, string> };
  }
  return cardBorderStyle(settings.value, settings.value.display.theme, scheme.value);
});

/** 套在根元素上的 CSS 變數：縮放、警示色、不透明度、邊框覆寫（算法與設定頁預覽共用） */
const rootStyle = computed(() => {
  if (settings.value === null) {
    return { "--scale": layout.value.scale.toFixed(4) };
  }
  return widgetRootStyle(settings.value, scheme.value, layout.value.scale);
});

/** 設定檔整份壞掉時的頂端警告 */
const bannerText = computed(() => {
  const fileWarning = warnings.value.find((warning) => warning.kind === "invalid_file");
  if (fileWarning === undefined) {
    return "";
  }
  if (fileWarning.field === "settings.toml") {
    return t("settings_warning.invalid_file", { line: fileWarning.line ?? 1 });
  }
  return t("settings_warning.invalid_file_field", { line: fileWarning.line ?? 1, field: fileWarning.field });
});

/** 這台 Windows 有沒有 Acrylic（玻璃主題的 CSS 依此決定卡片樣子） */
const materialAttribute = computed(() => (systemInfo.value.acrylic ? "acrylic" : "none"));

/** 套用一份新設定（含語言） */
function applySettings(newSettings: Settings) {
  settings.value = newSettings;
  locale.value = resolveLocale(newSettings.general.language, systemInfo.value.language);
}

/** 設定儲存（或 ⇄ 切換）之後：套用新設定，並重新取得讀檔警告（儲存成功後警告會清空） */
async function onSettingsSaved(newSettings: Settings) {
  applySettings(newSettings);
  const response = await getSettings();
  warnings.value = response.warnings;
}

/** ⇄：切換輪播／並排 */
async function toggleMode() {
  if (settings.value === null) {
    return;
  }
  const nextMode = settings.value.display.mode === "carousel" ? "overview" : "carousel";
  settings.value.display.mode = nextMode;
  await setDisplayMode(nextMode);
}

/** 右鍵選單（依目前語言建立；換語言時重建） */
async function rebuildContextMenu() {
  if (menuLocale === locale.value) {
    return;
  }
  menuLocale = locale.value;
  await buildContextMenu([
    { id: "refresh", text: t("app.refresh"), action: () => void refreshNow() },
    { id: "mode", text: t("app.switch_mode"), action: () => void toggleMode() },
    { id: "settings", text: t("app.menu_settings"), action: () => void openSettings() },
    { id: "about", text: t("app.menu_about"), action: () => void openSettings("about") },
    "separator",
    { id: "quit", text: t("app.quit"), action: () => void quitApp() },
  ]);
}

function showContextMenu(event: MouseEvent) {
  void popupContextMenu(event.clientX, event.clientY);
}

/** 按住卡片空白處拖曳視窗（按鈕除外） */
function beginDrag(event: MouseEvent) {
  const target = event.target as HTMLElement;
  if (event.button !== 0 || target.closest("button, a, input") !== null) {
    return;
  }
  // 連點第二下不再拖曳，避免 Windows 把「雙擊標題列」當成最大化
  if (event.detail > 1) {
    return;
  }
  void startWindowDrag();
}

/** 視窗移動或縮放停下來後，請 Rust 記住目前顯示方式的位置大小（M-005；位置大小由 Rust 直接讀視窗） */
function scheduleBoundsSave() {
  window.clearTimeout(boundsTimer);
  boundsTimer = window.setTimeout(() => void saveWindowBounds(), BOUNDS_SAVE_DELAY_MS);
}

/** 重複開啟程式時：外框閃一下提示位置（Rust 同時把小工具暫時浮到最上層） */
function flash() {
  window.clearTimeout(flashTimer);
  flashing.value = true;
  flashTimer = window.setTimeout(() => (flashing.value = false), FLASH_DURATION_MS);
}

function onWindowResize() {
  // 瀏覽器預覽（網址 size=寬x高）：照指定大小排版，截圖比對用
  const frame = inTauri ? null : previewFrameSize();
  if (frame !== null) {
    windowWidth.value = frame.width;
    windowHeight.value = frame.height;
    document.body.dataset.previewFrame = "on";
    document.body.style.setProperty("--preview-width", `${frame.width}px`);
    document.body.style.setProperty("--preview-height", `${frame.height}px`);
    return;
  }
  windowWidth.value = window.innerWidth;
  windowHeight.value = window.innerHeight;
}

function onSystemSchemeChange(event: MediaQueryListEvent) {
  systemIsDark.value = event.matches;
}

watch(locale, () => void rebuildContextMenu());

// 瀏覽器預覽（網址 stage=1）：在小工具後面畫示意桌布，亮暗跟著小工具的色系
watch(scheme, (value) => {
  if (!inTauri && previewWantsStage()) {
    document.body.dataset.previewStage = value;
  }
}, { immediate: true });

onMounted(async () => {
  onWindowResize();
  darkModeQuery.addEventListener("change", onSystemSchemeChange);
  window.addEventListener("resize", onWindowResize);
  cleanupTasks.push(() => darkModeQuery.removeEventListener("change", onSystemSchemeChange));
  cleanupTasks.push(() => window.removeEventListener("resize", onWindowResize));

  // 先把事件接好，再讀資料，避免漏掉剛好發生的更新
  cleanupTasks.push(await onUsageUpdated((value) => (snapshot.value = value)));
  cleanupTasks.push(await onRefreshState((value) => (refreshing.value = value.running)));
  cleanupTasks.push(await onSettingsChanged((value) => void onSettingsSaved(value)));
  cleanupTasks.push(await onWindowMovedOrResized(scheduleBoundsSave));
  cleanupTasks.push(await onWidgetFlash(flash));

  const [settingsResponse, loadedSnapshot, loadedSystemInfo] = await Promise.all([
    getSettings(),
    getSnapshot(),
    getSystemInfo(),
  ]);
  systemInfo.value = loadedSystemInfo;
  warnings.value = settingsResponse.warnings;
  applySettings(settingsResponse.settings);
  if (snapshot.value === null) {
    snapshot.value = loadedSnapshot;
  }
  await rebuildContextMenu();

  clockTimer = window.setInterval(() => (now.value = Date.now()), CLOCK_TICK_MS);
});

onBeforeUnmount(() => {
  for (const cleanup of cleanupTasks) {
    cleanup();
  }
  window.clearInterval(clockTimer);
  window.clearTimeout(boundsTimer);
  window.clearTimeout(flashTimer);
});
</script>

<template>
  <main
    v-if="settings !== null && snapshot !== null"
    class="widget-shell"
    :class="{ flashing }"
    :data-theme="settings.display.theme"
    :data-scheme="scheme"
    :data-mode="settings.display.mode"
    :data-border="borderStyle.mode"
    :data-alert-frame="settings.borders.alert_frame ? 'on' : 'off'"
    :data-material="materialAttribute"
    :style="rootStyle"
    :aria-label="t('app.name')"
    @mousedown="beginDrag"
    @contextmenu.prevent="showContextMenu"
    @mouseenter="hovering = true"
    @mouseleave="hovering = false"
  >
    <p v-if="bannerText" class="settings-banner" :title="bannerText">{{ bannerText }}</p>

    <div class="cards-area">
      <ToolbarButtons
        :refreshing="refreshing"
        @refresh="refreshNow"
        @switch-mode="toggleMode"
        @settings="openSettings()"
      />
      <CarouselLayout
        v-if="settings.display.mode === 'carousel' && providers.length > 0"
        :providers="providers"
        :settings="settings"
        :scheme="scheme"
        :now="now"
        :layout="layout"
        :paused="hovering"
      />
      <OverviewLayout
        v-else-if="providers.length > 0"
        :providers="providers"
        :settings="settings"
        :scheme="scheme"
        :now="now"
        :layout="layout"
      />
      <article v-else class="provider-card empty-card">
        <div class="card-head reserve-toolbar">
          <span class="provider-name">{{ t("widget.empty_title") }}</span>
        </div>
        <div class="card-message">{{ t("widget.empty_hint") }}</div>
      </article>
    </div>
  </main>
</template>
