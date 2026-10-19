// 檔案：types/app.ts
// 用途：「程式狀態」與「電腦環境」的型別，欄位名稱與 Rust 端一致：
//       關於頁資訊（about.rs 的 AppInfo）、保險絲與看過的額度（scheduler/runtime.rs 的 RuntimeSummary）、
//       電腦環境（system_info.rs 的 SystemInfo：語言「自動」的結果、能不能用 Acrylic）。
// 誰會用到：services/backend.ts、小工具與設定視窗。

/** 這台電腦的環境（get_system_info） */
export interface SystemInfo {
  /** 語言設定為「自動」時要用的語言（Rust 依 Windows 顯示語言與地區判斷，FR-17） */
  language: "zh-TW" | "en";
  /** 玻璃主題能不能套 Acrylic 材質（Windows 11 22H2 以上） */
  acrylic: boolean;
}

/** 關於頁：程式名稱、兩碼版本（例如 v1.0）、授權 */
export interface AppInfo {
  name: string;
  version: string;
  license: string;
}

/** Antigravity 保險絲狀態（41「狀態檔」的 antigravity_fuse） */
export interface AntigravityFuse {
  tripped: boolean;
  /** 跳脫時間（epoch 秒） */
  tripped_at: number | null;
  /** 回應裡的模型對話次數；回應不是 JSON（無法確認）時為 null */
  num_turns: number | null;
}

/** get_runtime_state 的回傳：保險絲 */
export interface RuntimeSummary {
  antigravity_fuse: AntigravityFuse;
}
