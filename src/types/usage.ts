// 檔案：types/usage.ts
// 用途：統一用量格式在畫面端的型別，欄位名稱與 Rust（cache.rs、providers/mod.rs）一致（41「統一用量格式」）。
// 誰會用到：services/backend.ts 與所有小工具元件。

export type ProviderId = "claude" | "codex" | "antigravity";

/** 狀態代碼（41「狀態代碼」） */
export type StatusCode =
  | "pending"
  | "ok"
  | "disabled"
  | "not_installed"
  | "not_logged_in"
  | "offline"
  | "outdated"
  | "timeout"
  | "parse_error"
  | "no_quota"
  | "fuse_tripped"
  | "error";

/** 一個額度 */
export interface Quota {
  id: string;
  label_key: string;
  label_param: string | null;
  used_percent: number;
  resets_at: number | null;
  window_minutes: number | null;
}

/** 一張重置券（M-014） */
export interface ResetCredit {
  /** 到期時間（epoch 秒）；null 代表不會過期 */
  expires_at: number | null;
  /** CLI 提供的名稱（例如 "Full reset (Weekly + 5 hr)"），只當滑鼠停留提示；沒有時為 null */
  title: string | null;
}

/** 一家服務的重置券（M-014）；目前只有 Codex 會回報 */
export interface ResetCredits {
  /** CLI 回報的可用張數（明細可能比這個少） */
  available_count: number;
  /** 有明細的可用券 */
  credits: ResetCredit[];
}

/** 一家的用量 */
export interface ProviderUsage {
  id: ProviderId;
  name: string;
  enabled: boolean;
  plan: string | null;
  status: StatusCode;
  status_detail: string | null;
  last_attempt_at: number | null;
  last_success_at: number | null;
  quotas: Quota[];
  /** 重置券；null 代表這家不回報重置券 */
  reset_credits: ResetCredits | null;
}

/** 一次完整的用量資料（cache.json、usage-updated 事件） */
export interface UsageSnapshot {
  schema_version: number;
  updated_at: number | null;
  providers: ProviderUsage[];
}
