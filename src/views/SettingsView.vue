<!--
  檔案：views/SettingsView.vue
  用途：設定視窗（FR-9、43「設定視窗」）：左側「一般／外觀／服務／關於」四頁導覽、右側內容、底部固定「儲存」「取消」。
        - 開啟時向 Rust 取目前設定（含讀檔警告）、用量快取、保險絲與看過的額度、程式版本、電腦環境；
        - 所有修改先放在「設定草稿」，編輯中每停 0.3 秒請 Rust 即時檢查（check_settings），錯誤顯示在欄位下方；
          錯誤裡的範圍、選項、型別由畫面依目前語言組字（例如中文「1～120」、英文「1 to 120」，FR-17）；
        - 按「儲存」才寫檔並立即套用（Rust 的 save_settings）；按「取消」不寫檔、直接關閉視窗；
        - 外觀用簡約主題配色，依草稿裡的色系顯示亮或暗；語言也跟著草稿即時切換
          （「自動」時用 Rust 依 Windows 顯示語言與地區判斷的結果）。
  誰會用到：App.vue（網址帶 ?settings 時）。
-->
<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, provide, reactive, ref, toRaw, watch } from "vue";
import { useI18n } from "vue-i18n";
import AboutPage from "../components/settings/AboutPage.vue";
import AppearancePage from "../components/settings/AppearancePage.vue";
import GeneralPage from "../components/settings/GeneralPage.vue";
import ProvidersPage from "../components/settings/ProvidersPage.vue";
import { SETTINGS_FORM_KEY } from "../components/settings/form-context";
import { resolveLocale } from "../i18n";
import { warningParams } from "../i18n/format";
import {
  checkSettings,
  closeSettings,
  getAppInfo,
  getRuntimeState,
  getSettings,
  getSnapshot,
  getSystemInfo,
  onSettingsOpenTab,
  onUsageUpdated,
  saveSettings,
  type SettingsTab,
} from "../services/backend";
import type { AppInfo, RuntimeSummary, SystemInfo } from "../types/app";
import type { SaveSettingsError, Settings, SettingsWarning } from "../types/settings";
import type { UsageSnapshot } from "../types/usage";
import { resolveScheme } from "../utils/appearance";
import { NUMBER_RANGES, type NumberField, warningFor } from "../utils/settings-form";

/** 停止編輯多久之後才請 Rust 檢查（避免每打一個字就檢查一次） */
const CHECK_DELAY_MS = 300;
/** 「已儲存」訊息顯示多久 */
const SAVED_MESSAGE_MS = 3000;
const TABS: SettingsTab[] = ["general", "appearance", "providers", "about"];

const { t, locale } = useI18n();

const activeTab = ref<SettingsTab>(initialTab());
const draft = ref<Settings | null>(null);
const loadWarnings = ref<SettingsWarning[]>([]);
const liveWarnings = ref<SettingsWarning[]>([]);
/** 數字輸入框看不懂的欄位（例如空白、小數）：欄位名稱 → true */
const localErrors = reactive<Record<string, boolean>>({});
const snapshot = ref<UsageSnapshot | null>(null);
const runtime = ref<RuntimeSummary | null>(null);
const appInfo = ref<AppInfo | null>(null);
/** 電腦環境：語言「自動」的結果、有沒有 Acrylic（預覽用） */
const systemInfo = ref<SystemInfo>({ language: "en", acrylic: false });
const loadFailed = ref(false);
const saving = ref(false);
const statusText = ref("");
const statusIsError = ref(false);

const darkModeQuery = window.matchMedia("(prefers-color-scheme: dark)");
const systemIsDark = ref(darkModeQuery.matches);
const cleanupTasks: Array<() => void> = [];
let checkTimer = 0;
let statusTimer = 0;

/** 網址的 tab 參數決定一開始顯示哪一頁（Rust 只會帶允許的值，這裡再確認一次） */
function initialTab(): SettingsTab {
  const requested = new URLSearchParams(location.search).get("tab");
  for (const tab of TABS) {
    if (tab === requested) {
      return tab;
    }
  }
  return "general";
}

/** 設定視窗用簡約主題配色，亮暗跟著草稿的色系（不受主題影響；霓虹的「僅暗色」只管小工具） */
const scheme = computed(() => {
  if (draft.value === null) {
    return systemIsDark.value ? "dark" : "light";
  }
  return resolveScheme("minimal", draft.value.display.color_scheme, systemIsDark.value);
});

const hasLocalErrors = computed(() => Object.keys(localErrors).length > 0);

// ---------------------------------------------------------------------------
// 欄位錯誤（提供給各頁的欄位元件）
// ---------------------------------------------------------------------------

/** 某欄位下方要顯示的錯誤文字：先看輸入框本身的問題，再看 Rust 的檢查結果 */
function errorFor(field: string): string {
  if (localErrors[field] === true) {
    // 輸入框裡不是整數：提示這一欄的合法範圍（範圍來自 settings-form.ts，與 Rust 相同）
    const range = NUMBER_RANGES[field as NumberField];
    if (range !== undefined) {
      return t("field_error.not_integer", { min: range.min, max: range.max });
    }
    return t("field_error.wrong_type", { type: t("value_type.integer") });
  }
  const warning = warningFor(field, liveWarnings.value);
  if (warning === undefined) {
    return "";
  }
  return t(`field_error.${warning.kind}`, warningParams(t, warning));
}

/** 讀檔警告的一行說明（範圍、選項、型別依目前語言組字） */
function loadWarningText(warning: SettingsWarning): string {
  return t(`settings_warning.${warning.kind}`, warningParams(t, warning));
}

function setLocalError(field: string, hasError: boolean) {
  if (hasError) {
    localErrors[field] = true;
  } else {
    delete localErrors[field];
  }
}

provide(SETTINGS_FORM_KEY, { errorFor, setLocalError });

// ---------------------------------------------------------------------------
// 即時檢查、儲存、取消
// ---------------------------------------------------------------------------

/** 請 Rust 檢查目前草稿（規則和儲存時相同） */
async function runCheck() {
  if (draft.value === null || hasLocalErrors.value) {
    return;
  }
  try {
    liveWarnings.value = await checkSettings(toRaw(draft.value));
  } catch {
    // 草稿暫時無法送出檢查（例如型別不符）時略過，儲存時仍會完整檢查
  }
}

function scheduleCheck() {
  window.clearTimeout(checkTimer);
  checkTimer = window.setTimeout(() => void runCheck(), CHECK_DELAY_MS);
}

function showStatus(text: string, isError: boolean) {
  window.clearTimeout(statusTimer);
  statusText.value = text;
  statusIsError.value = isError;
  if (!isError) {
    statusTimer = window.setTimeout(() => (statusText.value = ""), SAVED_MESSAGE_MS);
  }
}

async function save() {
  if (draft.value === null || saving.value) {
    return;
  }
  if (hasLocalErrors.value) {
    showStatus(t("settings.has_errors"), true);
    return;
  }

  saving.value = true;
  showStatus(t("settings.saving"), false);
  try {
    const saved = await saveSettings(toRaw(draft.value));
    draft.value = structuredClone(saved);
    loadWarnings.value = [];
    liveWarnings.value = [];
    showStatus(t("settings.saved"), false);
  } catch (reason) {
    const failure = reason as Partial<SaveSettingsError>;
    if (Array.isArray(failure.warnings) && failure.warnings.length > 0) {
      liveWarnings.value = failure.warnings;
      showStatus(t("settings.has_errors"), true);
    } else {
      const message = failure.message ?? String(reason);
      showStatus(t("settings.save_failed", { message }), true);
    }
  } finally {
    saving.value = false;
  }
}

function cancel() {
  void closeSettings();
}

/** 保險絲恢復後重新讀取狀態 */
async function reloadRuntime() {
  runtime.value = await getRuntimeState();
}

// 語言跟著草稿即時切換（按取消就整個視窗關掉，不影響小工具）
watch(
  () => draft.value?.general.language,
  (language) => {
    if (language !== undefined) {
      locale.value = resolveLocale(language, systemInfo.value.language);
    }
  },
);
watch(draft, scheduleCheck, { deep: true });

function onSystemSchemeChange(event: MediaQueryListEvent) {
  systemIsDark.value = event.matches;
}

onMounted(async () => {
  darkModeQuery.addEventListener("change", onSystemSchemeChange);
  cleanupTasks.push(() => darkModeQuery.removeEventListener("change", onSystemSchemeChange));
  // 先把事件接好：查詢完成時更新額度清單；已開著時按右鍵「關於…」切到關於頁
  cleanupTasks.push(await onUsageUpdated((value) => (snapshot.value = value)));
  cleanupTasks.push(await onSettingsOpenTab((tab) => (activeTab.value = tab)));

  try {
    const [settingsResponse, loadedSnapshot, loadedRuntime, loadedAppInfo, loadedSystemInfo] = await Promise.all([
      getSettings(),
      getSnapshot(),
      getRuntimeState(),
      getAppInfo(),
      getSystemInfo(),
    ]);
    // 先有電腦環境，草稿的語言 watch 才會用正確的「自動」結果
    systemInfo.value = loadedSystemInfo;
    draft.value = structuredClone(settingsResponse.settings);
    loadWarnings.value = settingsResponse.warnings;
    if (snapshot.value === null) {
      snapshot.value = loadedSnapshot;
    }
    runtime.value = loadedRuntime;
    appInfo.value = loadedAppInfo;
  } catch {
    loadFailed.value = true;
  }
});

onBeforeUnmount(() => {
  for (const cleanup of cleanupTasks) {
    cleanup();
  }
  window.clearTimeout(checkTimer);
  window.clearTimeout(statusTimer);
});
</script>

<template>
  <main class="settings-shell" data-theme="minimal" :data-scheme="scheme">
    <nav class="settings-nav" :aria-label="t('settings.nav_label')">
      <button
        v-for="tab in TABS"
        :key="tab"
        type="button"
        :aria-current="activeTab === tab ? 'page' : undefined"
        @click="activeTab = tab"
      >
        {{ t(`settings.tabs.${tab}`) }}
      </button>
    </nav>

    <section class="settings-content">
      <p v-if="loadFailed" class="field-error">{{ t("settings.load_error") }}</p>
      <p v-else-if="draft === null" class="muted">{{ t("app.loading") }}</p>
      <template v-else>
        <div v-if="loadWarnings.length > 0" class="load-warnings" role="alert">
          <p>{{ t("settings.load_warnings_title") }}</p>
          <ul>
            <li v-for="warning in loadWarnings" :key="warning.field">
              {{ loadWarningText(warning) }}
            </li>
          </ul>
        </div>

        <!-- 四頁都保持掛載、只切換顯示：換頁不會遺失輸入中的文字與偵測結果 -->
        <GeneralPage v-show="activeTab === 'general'" :draft="draft" />
        <AppearancePage
          v-show="activeTab === 'appearance'"
          :draft="draft"
          :system-is-dark="systemIsDark"
          :acrylic="systemInfo.acrylic"
        />
        <ProvidersPage
          v-show="activeTab === 'providers'"
          :draft="draft"
          :snapshot="snapshot"
          :runtime="runtime"
          @runtime-changed="reloadRuntime"
        />
        <AboutPage v-show="activeTab === 'about'" :app-info="appInfo" />
      </template>
    </section>

    <footer class="settings-footer">
      <p class="settings-status" :class="{ error: statusIsError }" role="status">{{ statusText }}</p>
      <button type="button" class="button" @click="cancel">{{ t("settings.cancel") }}</button>
      <button type="button" class="button primary" :disabled="draft === null || saving" @click="save">
        {{ t("settings.save") }}
      </button>
    </footer>
  </main>
</template>
