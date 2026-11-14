// 檔案：utils/reset-credits.test.ts
// 用途：驗證重置券（M-014）的卡片摘要（張數＋最快到期）與小視窗清單（依到期時間排序、不會過期排最後、
//       沒有明細的另外計數、已過期的不算）。
// 誰會用到：`npm run test`（Vitest）。

import { describe, expect, it } from "vitest";
import type { ProviderId, ProviderUsage, ResetCredits } from "../types/usage";
import { creditList, creditSummary } from "./reset-credits";

/** 現在時間：epoch 1_800_000_000 秒 */
const NOW_SECONDS = 1_800_000_000;
const NOW_MS = NOW_SECONDS * 1000;
const DAY = 86_400;

/** 建立一家只有重置券資料的用量 */
function provider(id: ProviderId, name: string, resetCredits: ResetCredits | null): ProviderUsage {
  return {
    id,
    name,
    enabled: true,
    plan: null,
    status: "ok",
    status_detail: null,
    last_attempt_at: NOW_SECONDS,
    last_success_at: NOW_SECONDS,
    quotas: [],
    reset_credits: resetCredits,
  };
}

/** 一張券：到期時間（距現在幾天；null＝不會過期） */
function credit(daysFromNow: number | null) {
  const expiresAt = daysFromNow === null ? null : NOW_SECONDS + daysFromNow * DAY;
  return { expires_at: expiresAt, title: null };
}

describe("卡片摘要 creditSummary", () => {
  it("2 張：張數 2、時間是最快到期的那張（不管 CLI 回報的順序）", () => {
    const codex = provider("codex", "Codex", { available_count: 2, credits: [credit(24), credit(13)] });
    expect(creditSummary(codex, NOW_MS)).toEqual({
      count: 2,
      expiryKind: "time",
      earliestExpiresAt: NOW_SECONDS + 13 * DAY,
    });
  });

  it("不回報重置券或 0 張：不顯示那一行", () => {
    expect(creditSummary(provider("claude", "Claude", null), NOW_MS)).toBeNull();
    expect(creditSummary(provider("codex", "Codex", { available_count: 0, credits: [] }), NOW_MS)).toBeNull();
  });

  it("已過期但還沒重新查詢的券不算：2 張過期 1 張 → 剩 1 張", () => {
    const codex = provider("codex", "Codex", { available_count: 2, credits: [credit(-1), credit(5)] });
    const summary = creditSummary(codex, NOW_MS);
    expect(summary?.count).toBe(1);
    expect(summary?.earliestExpiresAt).toBe(NOW_SECONDS + 5 * DAY);

    // 全部過期：不顯示
    const allExpired = provider("codex", "Codex", { available_count: 1, credits: [credit(-1)] });
    expect(creditSummary(allExpired, NOW_MS)).toBeNull();
  });

  it("每張都不會過期 → never；只有張數沒有明細 → unknown", () => {
    const never = provider("codex", "Codex", { available_count: 1, credits: [credit(null)] });
    expect(creditSummary(never, NOW_MS)?.expiryKind).toBe("never");

    const countOnly = provider("codex", "Codex", { available_count: 3, credits: [] });
    expect(creditSummary(countOnly, NOW_MS)).toEqual({ count: 3, expiryKind: "unknown", earliestExpiresAt: null });
  });
});

describe("小視窗清單 creditList", () => {
  it("所有服務的券依到期時間排序，快到期的在上面、不會過期的排最後", () => {
    const providers = [
      provider("claude", "Claude", { available_count: 2, credits: [credit(20), credit(null)] }),
      provider("codex", "Codex", { available_count: 2, credits: [credit(24), credit(3)] }),
      provider("antigravity", "Antigravity", null),
    ];
    const list = creditList(providers, NOW_MS);
    expect(list.total).toBe(4);
    expect(list.items.map((item) => [item.providerName, item.expiresAt])).toEqual([
      ["Codex", NOW_SECONDS + 3 * DAY],
      ["Claude", NOW_SECONDS + 20 * DAY],
      ["Codex", NOW_SECONDS + 24 * DAY],
      ["Claude", null],
    ]);
    expect(list.undetailed).toEqual([]);
  });

  it("張數比明細多：多出來的列為「沒有到期時間」；已過期的不列出", () => {
    const providers = [provider("codex", "Codex", { available_count: 4, credits: [credit(2), credit(-3)] })];
    const list = creditList(providers, NOW_MS);
    // 4 張扣掉 1 張過期 = 3 張；有明細的 1 張，另外 2 張沒有到期時間
    expect(list.total).toBe(3);
    expect(list.items).toHaveLength(1);
    expect(list.undetailed).toEqual([{ providerId: "codex", providerName: "Codex", count: 2 }]);
  });

  it("完全沒有券：清單是空的、總數 0", () => {
    const list = creditList([provider("codex", "Codex", { available_count: 0, credits: [] })], NOW_MS);
    expect(list).toEqual({ items: [], undetailed: [], total: 0 });
  });
});
