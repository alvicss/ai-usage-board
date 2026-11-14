// 檔案：utils/reset-credits.ts
// 用途：重置券（M-014）的純計算，不含顯示文字（文字在 i18n/format.ts）：
//       1. 卡片那一行的摘要：目前可用幾張、最快到期的那張是什麼時候（creditSummary）；
//       2. 重置券小視窗的清單：所有服務的券依到期時間排序，快到期的在上面、不會過期的排最後；
//          CLI 只給張數、沒給到期時間的另外計數（creditList）。
//       已經過了到期時間、但還沒重新查詢的券，當成已經不能用，不算進張數。
// 誰會用到：ProviderCard（卡片那一行）、ResetCreditsView（小視窗清單）。

import type { ProviderId, ProviderUsage, ResetCredit } from "../types/usage";

/** 最快到期時間的種類：有時間／全部都不會過期／CLI 沒給到期時間 */
export type ExpiryKind = "time" | "never" | "unknown";

/** 卡片上的重置券摘要 */
export interface CreditSummary {
  /** 目前可用的張數 */
  count: number;
  /** 最快到期時間的種類 */
  expiryKind: ExpiryKind;
  /** 最快到期的時間（epoch 秒）；expiryKind 不是 "time" 時為 null */
  earliestExpiresAt: number | null;
}

/** 小視窗清單裡的一張券 */
export interface CreditListItem {
  providerId: ProviderId;
  /** 服務名稱（Codex／Claude，不翻譯） */
  providerName: string;
  /** 到期時間（epoch 秒）；null 代表不會過期 */
  expiresAt: number | null;
  /** CLI 提供的名稱（滑鼠停留提示用） */
  title: string | null;
}

/** 小視窗清單：CLI 只給張數、沒有到期時間的券 */
export interface UndetailedCredits {
  providerId: ProviderId;
  providerName: string;
  count: number;
}

/** 小視窗要顯示的全部內容 */
export interface CreditList {
  /** 有到期時間（或確定不會過期）的券，已排好順序 */
  items: CreditListItem[];
  /** 各服務沒有明細的張數（只列出大於 0 的） */
  undetailed: UndetailedCredits[];
  /** 全部可用張數（標題旁的數字） */
  total: number;
}

/** 這張券現在還能不能用：不會過期，或到期時間還沒到 */
function isUsable(credit: ResetCredit, nowMilliseconds: number): boolean {
  if (credit.expires_at === null) {
    return true;
  }
  return credit.expires_at * 1000 > nowMilliseconds;
}

/** 一家的可用張數與可用的明細；這家不回報重置券時回傳 null */
function usableCreditsOf(
  provider: ProviderUsage,
  nowMilliseconds: number,
): { count: number; usable: ResetCredit[] } | null {
  const resetCredits = provider.reset_credits;
  if (resetCredits === null) {
    return null;
  }

  const usable: ResetCredit[] = [];
  for (const credit of resetCredits.credits) {
    if (isUsable(credit, nowMilliseconds)) {
      usable.push(credit);
    }
  }

  // 已過期的明細從張數扣掉（CLI 回報的張數是查詢當下的）
  const expiredCount = resetCredits.credits.length - usable.length;
  const count = Math.max(0, resetCredits.available_count - expiredCount);
  return { count, usable };
}

/**
 * 卡片那一行的摘要。這家不回報重置券、或目前沒有可用的券時回傳 null（卡片不顯示那一行）。
 * 輸入：一家的用量、現在時間（毫秒）。
 */
export function creditSummary(provider: ProviderUsage, nowMilliseconds: number): CreditSummary | null {
  const result = usableCreditsOf(provider, nowMilliseconds);
  if (result === null || result.count === 0) {
    return null;
  }

  // 找出有到期時間的券裡最早的那張
  let earliest: number | null = null;
  for (const credit of result.usable) {
    if (credit.expires_at === null) {
      continue;
    }
    if (earliest === null || credit.expires_at < earliest) {
      earliest = credit.expires_at;
    }
  }

  if (earliest !== null) {
    return { count: result.count, expiryKind: "time", earliestExpiresAt: earliest };
  }
  if (result.usable.length > 0) {
    // 有明細但每張都不會過期
    return { count: result.count, expiryKind: "never", earliestExpiresAt: null };
  }
  // 只有張數、沒有明細
  return { count: result.count, expiryKind: "unknown", earliestExpiresAt: null };
}

/** 排序用：不會過期的排在最後 */
function sortKey(item: CreditListItem): number {
  if (item.expiresAt === null) {
    return Number.POSITIVE_INFINITY;
  }
  return item.expiresAt;
}

/**
 * 小視窗的清單：所有服務的券依到期時間由近到遠排序（同時間時維持服務的固定順序）。
 * 輸入：要列出的服務（呼叫端先濾掉停用的）、現在時間（毫秒）。
 */
export function creditList(providers: ProviderUsage[], nowMilliseconds: number): CreditList {
  const items: CreditListItem[] = [];
  const undetailed: UndetailedCredits[] = [];
  let total = 0;

  for (const provider of providers) {
    const result = usableCreditsOf(provider, nowMilliseconds);
    if (result === null) {
      continue;
    }
    total += result.count;

    for (const credit of result.usable) {
      items.push({
        providerId: provider.id,
        providerName: provider.name,
        expiresAt: credit.expires_at,
        title: credit.title,
      });
    }

    // 張數比明細多：多出來的是 CLI 沒給到期時間的券
    const withoutDetail = result.count - result.usable.length;
    if (withoutDetail > 0) {
      undetailed.push({ providerId: provider.id, providerName: provider.name, count: withoutDetail });
    }
  }

  // Array.prototype.sort 是穩定排序：到期時間相同時，維持服務的固定順序
  items.sort((first, second) => sortKey(first) - sortKey(second));
  return { items, undetailed, total };
}
