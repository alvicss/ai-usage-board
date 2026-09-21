// 檔案：i18n/i18n.test.ts
// 用途：檢查繁中與英文翻譯檔的 key 完全一致（FR-17 驗收）、每個狀態代碼與設定警告種類都有畫面文字（41「狀態代碼」），
//       語言「自動」的換算，以及設定警告的範圍／選項／型別會依語言組字（英文不會出現全形「～」「、」）。
// 誰會用到：`npm run test`（Vitest）。

import { describe, expect, it } from "vitest";
import en from "./en.json";
import zhTW from "./zh-TW.json";
import { warningParams } from "./format";
import { i18n, resolveLocale } from "./index";
import type { SettingsWarning } from "../types/settings";

/** 把巢狀的翻譯物件攤平成 "a.b.c" 形式的 key 清單 */
function flattenKeys(value: unknown, prefix: string): string[] {
  if (typeof value !== "object" || value === null) {
    return [prefix];
  }
  const keys: string[] = [];
  for (const [key, child] of Object.entries(value)) {
    const path = prefix === "" ? key : `${prefix}.${key}`;
    for (const childKey of flattenKeys(child, path)) {
      keys.push(childKey);
    }
  }
  return keys;
}

/** 依 "a.b.c" 形式的 key 取出翻譯文字 */
function valueAt(messages: unknown, key: string): string {
  let node = messages;
  for (const part of key.split(".")) {
    node = (node as Record<string, unknown>)[part];
  }
  return String(node);
}

/** 用真的翻譯檔翻譯（指定語言） */
function translator(locale: "zh-TW" | "en") {
  return (key: string, named: Record<string, unknown>) => {
    i18n.global.locale.value = locale;
    return i18n.global.t(key, named);
  };
}

/** 做一筆測試用的設定警告 */
function makeWarning(partial: Partial<SettingsWarning>): SettingsWarning {
  return {
    field: "general.refresh_interval_minutes",
    kind: "out_of_range",
    line: null,
    expected: "1～120",
    fallback: "10",
    minimum: null,
    maximum: null,
    choices: [],
    value_type: null,
    ...partial,
  };
}

const statusCodes = [
  "pending",
  "ok",
  "disabled",
  "not_installed",
  "not_logged_in",
  "offline",
  "outdated",
  "timeout",
  "parse_error",
  "no_quota",
  "fuse_tripped",
  "error",
];

/** Rust settings/warning.rs 的所有警告種類 */
const warningKinds = [
  "invalid_file",
  "wrong_type",
  "out_of_range",
  "invalid_choice",
  "invalid_color",
  "invalid_border",
  "invalid_cli_path",
  "unsupported_schema",
];

describe("翻譯檔", () => {
  it("繁中與英文的 key 完全一致", () => {
    const chineseKeys = flattenKeys(zhTW, "").sort();
    const englishKeys = flattenKeys(en, "").sort();
    expect(chineseKeys).toEqual(englishKeys);
  });

  it("每個狀態代碼都有原因與解決方法文字", () => {
    const keys = flattenKeys(zhTW, "");
    for (const code of statusCodes) {
      expect(keys).toContain(`status.${code}`);
      expect(keys).toContain(`status_help.${code}`);
    }
  });

  it("每種設定警告都有讀檔警告與欄位錯誤的文字", () => {
    const keys = flattenKeys(zhTW, "");
    for (const kind of warningKinds) {
      expect(keys).toContain(`settings_warning.${kind}`);
      expect(keys).toContain(`field_error.${kind}`);
    }
  });

  it("英文翻譯裡沒有中文字與全形標點（作者名稱「半桶水的AI筆記」除外）", () => {
    for (const key of flattenKeys(en, "")) {
      if (key === "_file" || key === "settings.about.author_name") {
        continue;
      }
      expect(valueAt(en, key), key).not.toMatch(/[　-鿿＀-￯]/);
    }
  });
});

describe("resolveLocale", () => {
  it("auto 時用 Rust 依 Windows 顯示語言與地區判斷的結果", () => {
    expect(resolveLocale("auto", "zh-TW")).toBe("zh-TW");
    expect(resolveLocale("auto", "en")).toBe("en");
  });

  it("瀏覽器預覽時用瀏覽器語言：中文開頭用繁中、其他用英文", () => {
    expect(resolveLocale("auto", "zh-CN")).toBe("zh-TW");
    expect(resolveLocale("auto", "en-US")).toBe("en");
    expect(resolveLocale("auto", "ja-JP")).toBe("en");
  });

  it("手動選的語言優先", () => {
    expect(resolveLocale("en", "zh-TW")).toBe("en");
    expect(resolveLocale("zh-TW", "en")).toBe("zh-TW");
  });
});

describe("設定警告依語言組字（FR-17）", () => {
  it("範圍：中文「1～120」、英文「from 1 to 120」", () => {
    const warning = makeWarning({ minimum: 1, maximum: 120 });
    const zh = translator("zh-TW");
    expect(zh("field_error.out_of_range", warningParams(zh, warning))).toBe("請填 1～120 之間的整數");
    const english = translator("en");
    expect(english("field_error.out_of_range", warningParams(english, warning))).toBe(
      "Enter a whole number from 1 to 120",
    );
  });

  it("選項：中文用頓號、英文用逗號", () => {
    const warning = makeWarning({
      field: "display.color_scheme",
      kind: "invalid_choice",
      choices: ["auto", "light", "dark"],
      fallback: "auto",
    });
    const zh = translator("zh-TW");
    expect(zh("settings_warning.invalid_choice", warningParams(zh, warning))).toContain("auto、light、dark");
    const english = translator("en");
    expect(english("settings_warning.invalid_choice", warningParams(english, warning))).toContain(
      "auto, light, dark",
    );
  });

  it("型別：翻成該語言的型別名稱", () => {
    const warning = makeWarning({ field: "general.autostart", kind: "wrong_type", value_type: "boolean", fallback: "false" });
    const english = translator("en");
    expect(english("field_error.wrong_type", warningParams(english, warning))).toBe("Wrong format (expected true or false)");
    const zh = translator("zh-TW");
    expect(zh("field_error.wrong_type", warningParams(zh, warning))).toBe("格式不正確（應為 true／false）");
  });
});
