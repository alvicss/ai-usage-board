// 檔案：utils/settings-form.test.ts
// 用途：驗證設定畫面的純計算：預設值與 default-settings.toml 一致、邊框模式換算（FR-18）、
//       欄位警告查找、「新」額度判斷（FR-12）、整數解析。
// 誰會用到：`npm run test`（Vitest）。

import { describe, expect, it } from "vitest";
// 內建預設設定檔全文（Rust 編譯時嵌入的那一份）；?raw 代表以純文字載入
import defaultSettingsText from "../../src-tauri/config/default-settings.toml?raw";
import type { SettingsWarning } from "../types/settings";
import {
  DEFAULT_ALERT_COLOR,
  DEFAULT_COLORS,
  NUMBER_RANGES,
  borderFieldKey,
  borderModeOf,
  borderValueFor,
  isNewQuota,
  parseInteger,
  warningFor,
} from "./settings-form";

/**
 * 從預設設定檔取出某一行 `key = value` 的值（只比對第一個出現的 key）。
 * 有引號的值取引號內的文字（顏色 "#01B468" 含 #，不能當成註解）；沒引號的值取到空白或 # 為止。
 */
function defaultValue(key: string): string {
  const pattern = new RegExp(`^${key}\\s*=\\s*(?:"([^"]*)"|([^\\s#]+))`, "m");
  const match = defaultSettingsText.match(pattern);
  if (match === null) {
    throw new Error(`預設設定檔找不到 ${key}`);
  }
  if (match[1] !== undefined) {
    return match[1];
  }
  return match[2];
}

describe("還原預設用的預設值", () => {
  it("進度條配色與 default-settings.toml 一致", () => {
    expect(defaultValue("low")).toBe(DEFAULT_COLORS.low);
    expect(defaultValue("mid")).toBe(DEFAULT_COLORS.mid);
    expect(defaultValue("high")).toBe(DEFAULT_COLORS.high);
    expect(Number(defaultValue("mid_position"))).toBe(DEFAULT_COLORS.mid_position);
  });

  it("警示顏色與 default-settings.toml 一致", () => {
    expect(defaultValue("alert_color")).toBe(DEFAULT_ALERT_COLOR);
  });

  it("數字範圍與設定檔註解寫的範圍相同", () => {
    const pairs: Array<[string, { min: number; max: number }]> = [
      ["refresh_interval_minutes", NUMBER_RANGES["general.refresh_interval_minutes"]],
      ["opacity_percent", NUMBER_RANGES["display.opacity_percent"]],
      ["mid_position", NUMBER_RANGES["colors.mid_position"]],
      ["log_retention_days", NUMBER_RANGES["general.log_retention_days"]],
    ];
    for (const [key, range] of pairs) {
      // 例：opacity_percent = 100   # 整體不透明度 30～100 / Widget opacity
      const pattern = new RegExp(`^${key}\\s*=.*#.*?(\\d+)～(\\d+)`, "m");
      const match = defaultSettingsText.match(pattern);
      expect(match, key).not.toBeNull();
      expect(Number(match?.[1])).toBe(range.min);
      expect(Number(match?.[2])).toBe(range.max);
    }
    expect(NUMBER_RANGES["display.carousel_seconds"]).toEqual({ min: 3, max: 60 });
  });
});

describe("卡片邊框（FR-18）", () => {
  it("每個主題＋色系對應自己的欄位，霓虹固定暗色", () => {
    expect(borderFieldKey("minimal", "light")).toBe("minimal_light");
    expect(borderFieldKey("glass", "dark")).toBe("glass_dark");
    expect(borderFieldKey("neon", "light")).toBe("neon_dark");
  });

  it("設定值換成模式", () => {
    expect(borderModeOf("default")).toBe("default");
    expect(borderModeOf("none")).toBe("none");
    expect(borderModeOf("#12AB34")).toBe("custom");
    expect(borderModeOf("red")).toBe("default");
  });

  it("模式組回設定值", () => {
    expect(borderValueFor("default", "#123456")).toBe("default");
    expect(borderValueFor("none", "#123456")).toBe("none");
    expect(borderValueFor("custom", "#123456")).toBe("#123456");
  });
});

describe("欄位警告與新額度", () => {
  const warnings: SettingsWarning[] = [
    {
      field: "general.refresh_interval_minutes",
      kind: "out_of_range",
      line: null,
      expected: "1～120",
      fallback: "10",
      minimum: 1,
      maximum: 120,
      choices: [],
      value_type: null,
    },
  ];

  it("依欄位名稱找到警告", () => {
    expect(warningFor("general.refresh_interval_minutes", warnings)?.kind).toBe("out_of_range");
    expect(warningFor("general.log_retention_days", warnings)).toBeUndefined();
  });

  it("沒看過的額度標新", () => {
    expect(isNewQuota("codex.extra.x.primary", ["codex.primary"])).toBe(true);
    expect(isNewQuota("codex.primary", ["codex.primary"])).toBe(false);
  });
});

describe("parseInteger", () => {
  it("只接受整數", () => {
    expect(parseInteger("12")).toBe(12);
    expect(parseInteger(" 7 ")).toBe(7);
    expect(parseInteger("-3")).toBe(-3);
    expect(parseInteger("")).toBeNull();
    expect(parseInteger("1.5")).toBeNull();
    expect(parseInteger("10分")).toBeNull();
  });
});
