// 檔案：utils/quota.test.ts
// 用途：驗證額度勾選過濾（FR-12）、警示門檻判斷與輪播紅點（FR-7）、額度標籤文字。
// 誰會用到：`npm run test`（Vitest）。

import { describe, expect, it } from "vitest";
import { i18n } from "../i18n";
import { quotaLabel } from "../i18n/format";
import { previewSettingsResponse, previewSnapshot } from "../services/preview-data";
import type { Quota } from "../types/usage";
import { displayPercent, filledCellCount, isFailedStatus, isQuotaAlert, providerHasAlert, visibleQuotas } from "./quota";

function zhTranslate(key: string, named: Record<string, unknown>): string {
  i18n.global.locale.value = "zh-TW";
  return i18n.global.t(key, named);
}

function quota(labelKey: string, labelParam: string | null): Quota {
  return { id: "x", label_key: labelKey, label_param: labelParam, used_percent: 10, resets_at: null, window_minutes: null };
}

describe("visibleQuotas", () => {
  it("只顯示勾選的額度", () => {
    const settings = previewSettingsResponse().settings;
    const claude = previewSnapshot().providers[0];
    settings.providers.claude.visible_quotas = ["claude.seven_day"];
    const shown = visibleQuotas(claude, settings.providers.claude);
    expect(shown.map((item) => item.id)).toEqual(["claude.seven_day"]);
  });
});

describe("畫面顯示的整數百分比（進度條跟著數字走）", () => {
  it("四捨五入到整數：0.06 → 0、0.5 → 1、99.6 → 100", () => {
    expect(displayPercent(0.06)).toBe(0);
    expect(displayPercent(0.49)).toBe(0);
    expect(displayPercent(0.5)).toBe(1);
    expect(displayPercent(62.4)).toBe(62);
    expect(displayPercent(99.6)).toBe(100);
  });

  it("超出 0～100 時限制在範圍內", () => {
    expect(displayPercent(-3)).toBe(0);
    expect(displayPercent(130)).toBe(100);
  });

  it("字元條格數照整數算：0% 一定是 0 格", () => {
    expect(filledCellCount(displayPercent(0.06), 10)).toBe(0);
    expect(filledCellCount(0, 24)).toBe(0);
    expect(filledCellCount(50, 10)).toBe(5);
    expect(filledCellCount(100, 12)).toBe(12);
    expect(filledCellCount(62, 10)).toBe(6);
  });
});

describe("警示", () => {
  it("用顯示出來的整數判斷門檻", () => {
    expect(displayPercent(79.6)).toBe(80);
    expect(isQuotaAlert({ ...quota("quota.weekly", null), used_percent: 79.6 }, 80)).toBe(true);
    expect(isQuotaAlert({ ...quota("quota.weekly", null), used_percent: 79.4 }, 80)).toBe(false);
  });

  it("勾選顯示的額度超過門檻才亮警示；查詢失敗不亮", () => {
    const settings = previewSettingsResponse().settings;
    const codex = previewSnapshot().providers[1]; // 5 小時 88%
    expect(providerHasAlert(codex, settings)).toBe(true);

    settings.providers.codex.visible_quotas = ["codex.secondary"]; // 只顯示 48% 那一個
    expect(providerHasAlert(codex, settings)).toBe(false);

    const failedCodex = { ...codex, status: "offline" as const };
    settings.providers.codex.visible_quotas = ["codex.primary"];
    expect(isFailedStatus(failedCodex)).toBe(true);
    expect(providerHasAlert(failedCodex, settings)).toBe(false);
  });

  it("「此帳號沒有訂閱額度資料」和查詢失敗一樣：卡片轉灰、不亮警示", () => {
    const settings = previewSettingsResponse().settings;
    const codex = previewSnapshot().providers[1]; // 5 小時 88%
    const noQuotaCodex = { ...codex, status: "no_quota" as const };
    expect(isFailedStatus(noQuotaCodex)).toBe(true);
    expect(providerHasAlert(noQuotaCodex, settings)).toBe(false);
  });
});

describe("quotaLabel", () => {
  it("一般標籤與加上群組名稱的標籤", () => {
    expect(quotaLabel(zhTranslate, quota("quota.five_hour", null))).toBe("5 小時");
    expect(quotaLabel(zhTranslate, quota("quota.weekly", "Gemini"))).toBe("Gemini 每週");
    expect(quotaLabel(zhTranslate, quota("quota.five_hour", "Claude+GPT"))).toBe("Claude+GPT 5 小時");
  });

  it("自帶名稱參數的標籤", () => {
    expect(quotaLabel(zhTranslate, quota("quota.model_weekly", "Fable"))).toBe("Fable 每週");
    expect(quotaLabel(zhTranslate, quota("quota.window_hours", "3"))).toBe("3 小時");
  });
});
