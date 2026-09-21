// 檔案：services/dialog.ts
// 用途：Tauri 內建的「選擇檔案」對話框（dialog 外掛），用來挑 CLI 程式檔的位置：
//       只能選 .exe、.cmd；選到的路徑仍要經過 Rust 的設定檢查（檔案存在、副檔名正確）才會存檔（FR-11）。
//       本案自己的 Rust 指令一律在 backend.ts；這裡只放 Tauri 內建的對話框，元件不直接呼叫外掛。
// 誰會用到：設定視窗「服務」頁（ProviderSection）。

import { open } from "@tauri-apps/plugin-dialog";
import { inTauri } from "./backend";

/**
 * 開啟「選擇檔案」對話框，只顯示指定副檔名的檔案。
 * 輸入：篩選器名稱（已翻譯的文字）、允許的副檔名。
 * 輸出：選到的完整路徑；按取消（或在瀏覽器預覽）時回傳 null。
 */
async function pickFile(filterName: string, extensions: string[]): Promise<string | null> {
  if (!inTauri) {
    return null;
  }
  const selected = await open({
    multiple: false,
    directory: false,
    filters: [{ name: filterName, extensions }],
  });
  if (typeof selected === "string") {
    return selected;
  }
  return null;
}

/** 選 CLI 程式檔（.exe、.cmd） */
export function pickCliFile(filterName: string): Promise<string | null> {
  return pickFile(filterName, ["exe", "cmd"]);
}
