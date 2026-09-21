// 檔案：i18n/format.ts
// 用途：把資料組成「畫面上要顯示的文字」：額度標籤、重置時間（倒數／絕對時間／兩者；工程師與儀表主題用短格式）、
//       「N 分鐘前更新」、查詢失敗原因、設定警告的參數（範圍、選項、型別依語言組字，FR-17）。
//       數字計算在 utils/time.ts，文字一律來自翻譯檔。
// 誰會用到：ProviderCard、QuotaRow、TerminalRow、GaugeRing、設定畫面（欄位錯誤、讀檔警告、額度勾選清單）、WidgetView。

import type { ResetTimeFormat, SettingsWarning } from "../types/settings";
import type { ProviderId, ProviderUsage, Quota } from "../types/usage";
import { countdownParts, formatAbsolute, minutesSince } from "../utils/time";

/** 翻譯函式（vue-i18n 的 t），寫成最簡單的形狀，方便單元測試直接傳入 */
export type Translate = (key: string, named: Record<string, unknown>) => string;

/** 這些標籤前面要加上群組名稱，例如 Antigravity 的「Gemini 5 小時」 */
const PREFIX_LABEL_KEYS = ["quota.five_hour", "quota.weekly", "quota.unknown_window"];

/** 額度標籤，例如「5 小時」「Gemini 每週」「Fable 每週」 */
export function quotaLabel(t: Translate, quota: Quota): string {
  const labelParameter = quota.label_param ?? "";
  if (PREFIX_LABEL_KEYS.includes(quota.label_key)) {
    const baseLabel = t(quota.label_key, {});
    if (labelParameter === "") {
      return baseLabel;
    }
    return `${labelParameter} ${baseLabel}`;
  }
  // 其他標籤自己帶有 {name} 參數，例如「{name} 每週」「{name} 小時」
  return t(quota.label_key, { name: labelParameter });
}

/** 重置時間的顯示結果 */
export interface ResetText {
  /** 要顯示的文字；「已重置，等待更新」也放在這裡 */
  text: string;
  /** 重置時間已過、但還沒查到新資料 */
  expired: boolean;
}

/**
 * 重置時間文字（FR-5）。
 * 倒數：「2 小時 13 分後」，超過 24 小時「3 天 4 小時後」；絕對：「09/20 14:00」；兩者用「 · 」相連。
 * 沒有重置時間時回傳 null（不顯示那一行）。
 */
export function resetText(
  t: Translate,
  resetsAt: number | null,
  format: ResetTimeFormat,
  nowMilliseconds: number,
): ResetText | null {
  if (resetsAt === null) {
    return null;
  }

  const parts = countdownParts(resetsAt, nowMilliseconds);
  if (parts.expired) {
    return { text: t("time.expired", {}), expired: true };
  }

  let countdown: string;
  if (parts.days > 0) {
    countdown = t("time.countdown_days", { days: parts.days, hours: parts.hours });
  } else if (parts.hours > 0) {
    countdown = t("time.countdown_hours", { hours: parts.hours, minutes: parts.minutes });
  } else {
    countdown = t("time.countdown_minutes", { minutes: parts.minutes });
  }
  const absolute = formatAbsolute(resetsAt);

  if (format === "absolute") {
    return { text: absolute, expired: false };
  }
  if (format === "both") {
    return { text: t("time.both", { countdown, absolute }), expired: false };
  }
  return { text: countdown, expired: false };
}

/**
 * 短格式的重置時間（工程師主題每行右邊、儀表主題環下方用）：倒數寫成「2h13m」「3d4h」「45m」，
 * 絕對時間「09/20 14:00」，兩者用「 · 」相連；已重置時顯示「已重置，等待更新」。沒有重置時間回傳 null。
 * 「2h13m」這種寫法中英文相同（終端機風格），所以不經過翻譯檔。
 */
export function compactResetText(
  t: Translate,
  resetsAt: number | null,
  format: ResetTimeFormat,
  nowMilliseconds: number,
): ResetText | null {
  if (resetsAt === null) {
    return null;
  }

  const parts = countdownParts(resetsAt, nowMilliseconds);
  if (parts.expired) {
    return { text: t("time.expired", {}), expired: true };
  }

  let countdown: string;
  if (parts.days > 0) {
    countdown = `${parts.days}d${parts.hours}h`;
  } else if (parts.hours > 0) {
    countdown = `${parts.hours}h${String(parts.minutes).padStart(2, "0")}m`;
  } else {
    countdown = `${parts.minutes}m`;
  }
  const absolute = formatAbsolute(resetsAt);

  if (format === "absolute") {
    return { text: absolute, expired: false };
  }
  if (format === "both") {
    return { text: `${countdown} · ${absolute}`, expired: false };
  }
  return { text: countdown, expired: false };
}

/**
 * 設定警告要代入翻譯文字的參數（FR-17：範圍、選項、型別依語言組字，不直接顯示 Rust 的中文說明）。
 * 回傳：field（欄位）、min／max（範圍）、choices（選項，用該語言的頓號或逗號串起來）、
 *       type（型別名稱）、fallback（改用的值）、line（行號）。
 */
export function warningParams(t: Translate, warning: SettingsWarning): Record<string, unknown> {
  const separator = t("format.list_separator", {});
  const choices = warning.choices.join(separator);

  let typeName = "";
  if (warning.value_type !== null) {
    typeName = t(`value_type.${warning.value_type}`, {});
  }

  // 整個區段型別不對時，Rust 的 fallback 是中文說明，改用翻譯檔的文字
  let fallback = warning.fallback;
  if (warning.value_type === "table") {
    fallback = t("value_type.table_defaults", {});
  }

  return {
    field: warning.field,
    min: warning.minimum ?? "",
    max: warning.maximum ?? "",
    choices,
    type: typeName,
    fallback,
    line: warning.line ?? 1,
  };
}

/** 「N 分鐘前更新」（FR-6），用該家最後一次成功的時間 */
export function updatedText(t: Translate, lastSuccessAt: number | null, nowMilliseconds: number): string {
  const minutes = minutesSince(lastSuccessAt, nowMilliseconds);
  if (minutes === null) {
    return t("time.never", {});
  }
  if (minutes < 1) {
    return t("time.just_now", {});
  }
  if (minutes < 60) {
    return t("time.minutes_ago", { count: minutes });
  }
  const hours = Math.floor(minutes / 60);
  if (hours < 24) {
    return t("time.hours_ago", { count: hours });
  }
  const days = Math.floor(hours / 24);
  return t("time.days_ago", { count: days });
}

/** 各家 CLI 的指令名稱（「找不到 codex」這類提示用，照原文不翻譯） */
const CLI_NAMES: Record<ProviderId, string> = {
  claude: "claude",
  codex: "codex",
  antigravity: "agy",
};

/** 標題列下方的失敗原因（有舊資料時用，短句） */
export function statusReason(t: Translate, provider: ProviderUsage): string {
  return t(`status.${provider.status}`, { name: provider.name, cli: CLI_NAMES[provider.id] });
}

/** 從未成功過時，卡片中央的「原因＋怎麼解決」（FR-6） */
export function statusHelp(t: Translate, provider: ProviderUsage): string {
  return t(`status_help.${provider.status}`, { name: provider.name, cli: CLI_NAMES[provider.id] });
}
