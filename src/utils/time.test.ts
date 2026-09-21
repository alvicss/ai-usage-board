// 檔案：utils/time.test.ts
// 用途：驗證時間計算與三種重置時間格式（FR-5 驗收：三種格式正確、倒數每分鐘遞減）、工程師／儀表主題的短格式、「N 分鐘前更新」。
// 誰會用到：`npm run test`（Vitest）。

import { describe, expect, it } from "vitest";
import { compactResetText, resetText, updatedText } from "../i18n/format";
import { i18n } from "../i18n";
import { countdownParts, formatAbsolute, minutesSince } from "./time";

/** 用真的翻譯檔做翻譯，確認組出來的句子與 43 的格式一致 */
function translator(locale: "zh-TW" | "en") {
  return (key: string, named: Record<string, unknown>) => {
    i18n.global.locale.value = locale;
    return i18n.global.t(key, named);
  };
}

// 2026-09-20 10:00（本機時區）
const now = new Date(2026, 8, 20, 10, 0, 0).getTime();
const minute = 60;

describe("countdownParts", () => {
  it("拆成天／時／分，分鐘無條件進位", () => {
    const resetsAt = now / 1000 + (2 * 60 + 13) * minute;
    expect(countdownParts(resetsAt, now)).toEqual({ expired: false, days: 0, hours: 2, minutes: 13 });
    expect(countdownParts(now / 1000 + 30, now)).toEqual({ expired: false, days: 0, hours: 0, minutes: 1 });
    const threeDays = now / 1000 + (3 * 1440 + 4 * 60) * minute;
    expect(countdownParts(threeDays, now)).toMatchObject({ days: 3, hours: 4, minutes: 0 });
  });

  it("重置時間已過就算過期", () => {
    expect(countdownParts(now / 1000, now).expired).toBe(true);
    expect(countdownParts(now / 1000 - 1, now).expired).toBe(true);
  });

  it("倒數每過一分鐘少一分鐘", () => {
    const resetsAt = now / 1000 + 10 * minute;
    expect(countdownParts(resetsAt, now).minutes).toBe(10);
    expect(countdownParts(resetsAt, now + 60_000).minutes).toBe(9);
  });
});

describe("formatAbsolute", () => {
  it("本機時區、24 小時制的 MM/DD HH:mm", () => {
    const at = new Date(2026, 8, 20, 14, 0, 0).getTime() / 1000;
    expect(formatAbsolute(at)).toBe("09/20 14:00");
    const morning = new Date(2026, 0, 5, 9, 7, 0).getTime() / 1000;
    expect(formatAbsolute(morning)).toBe("01/05 09:07");
  });
});

describe("resetText（繁中）", () => {
  const t = translator("zh-TW");
  const resetsAt = now / 1000 + (2 * 60 + 13) * minute;

  it("倒數", () => {
    expect(resetText(t, resetsAt, "countdown", now)?.text).toBe("2 小時 13 分後");
  });

  it("超過 24 小時顯示天數", () => {
    const later = now / 1000 + (3 * 1440 + 4 * 60) * minute;
    expect(resetText(t, later, "countdown", now)?.text).toBe("3 天 4 小時後");
  });

  it("絕對時間與兩者", () => {
    expect(resetText(t, resetsAt, "absolute", now)?.text).toBe("09/20 12:13");
    expect(resetText(t, resetsAt, "both", now)?.text).toBe("2 小時 13 分後 · 09/20 12:13");
  });

  it("已過期顯示「已重置，等待更新」；沒有重置時間不顯示", () => {
    const expired = resetText(t, now / 1000 - 5, "countdown", now);
    expect(expired).toEqual({ text: "已重置，等待更新", expired: true });
    expect(resetText(t, null, "countdown", now)).toBeNull();
  });
});

describe("resetText（英文）", () => {
  it("英文倒數", () => {
    const t = translator("en");
    const resetsAt = now / 1000 + (2 * 60 + 13) * minute;
    expect(resetText(t, resetsAt, "countdown", now)?.text).toBe("in 2h 13m");
  });
});

describe("minutesSince／updatedText", () => {
  const t = translator("zh-TW");

  it("算出距今幾分鐘", () => {
    expect(minutesSince(null, now)).toBeNull();
    expect(minutesSince(now / 1000 - 3 * minute, now)).toBe(3);
    expect(minutesSince(now / 1000 + 10, now)).toBe(0);
  });

  it("組成「N 分鐘前更新」", () => {
    expect(updatedText(t, null, now)).toBe("尚未成功更新");
    expect(updatedText(t, now / 1000 - 20, now)).toBe("剛剛更新");
    expect(updatedText(t, now / 1000 - 3 * minute, now)).toBe("3 分鐘前更新");
    expect(updatedText(t, now / 1000 - 125 * minute, now)).toBe("2 小時前更新");
    expect(updatedText(t, now / 1000 - 2 * 1440 * minute, now)).toBe("2 天前更新");
  });
});

describe("compactResetText（工程師、儀表主題的短格式）", () => {
  const t = translator("zh-TW");

  it("倒數寫成 2h13m、3d4h、45m", () => {
    expect(compactResetText(t, now / 1000 + (2 * 60 + 13) * minute, "countdown", now)?.text).toBe("2h13m");
    expect(compactResetText(t, now / 1000 + (1 * 60 + 5) * minute, "countdown", now)?.text).toBe("1h05m");
    expect(compactResetText(t, now / 1000 + (3 * 1440 + 4 * 60) * minute, "countdown", now)?.text).toBe("3d4h");
    expect(compactResetText(t, now / 1000 + 45 * minute, "countdown", now)?.text).toBe("45m");
  });

  it("絕對時間、兩者、已過期", () => {
    const resetsAt = now / 1000 + (2 * 60 + 13) * minute;
    expect(compactResetText(t, resetsAt, "absolute", now)?.text).toBe("09/20 12:13");
    expect(compactResetText(t, resetsAt, "both", now)?.text).toBe("2h13m · 09/20 12:13");
    expect(compactResetText(t, now / 1000 - 5, "countdown", now)).toEqual({ text: "已重置，等待更新", expired: true });
    expect(compactResetText(t, null, "countdown", now)).toBeNull();
  });
});