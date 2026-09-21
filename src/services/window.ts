// 檔案：services/window.ts
// 用途：集中使用 Tauri 內建的「視窗與選單」功能：拖曳小工具、監聽移動／縮放（停下來後請 Rust 記住位置）、
//       原生右鍵選單（43：用 Tauri 原生選單，不在 Vue 裡畫，小工具縮很小時選單也不會被切掉）。
//       本案自己的 Rust 指令與事件一律在 backend.ts；這裡只放 Tauri 內建 API，元件不直接呼叫它們。
// 誰會用到：WidgetView。

import type { UnlistenFn } from "@tauri-apps/api/event";
import { LogicalPosition } from "@tauri-apps/api/dpi";
import { Menu } from "@tauri-apps/api/menu";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { inTauri } from "./backend";

/** 右鍵選單的一個項目；separator 為分隔線 */
export type ContextMenuEntry = { id: string; text: string; action: () => void } | "separator";

/** 目前建好的右鍵選單（換語言時重建，舊的要關掉釋放資源） */
let contextMenu: Menu | null = null;

/** 按住小工具空白處時開始拖曳視窗 */
export async function startWindowDrag(): Promise<void> {
  if (!inTauri) {
    return;
  }
  await getCurrentWindow().startDragging();
}

/** 視窗被移動或縮放時通知 handler；回傳取消監聽的函式 */
export async function onWindowMovedOrResized(handler: () => void): Promise<UnlistenFn> {
  if (!inTauri) {
    return () => undefined;
  }
  const currentWindow = getCurrentWindow();
  const stopMoved = await currentWindow.onMoved(handler);
  const stopResized = await currentWindow.onResized(handler);
  return () => {
    stopMoved();
    stopResized();
  };
}

/** 建立（或重建）右鍵選單 */
export async function buildContextMenu(entries: ContextMenuEntry[]): Promise<void> {
  if (!inTauri) {
    return;
  }

  const items = [];
  for (const entry of entries) {
    if (entry === "separator") {
      items.push({ item: "Separator" as const });
    } else {
      items.push({ id: entry.id, text: entry.text, action: entry.action });
    }
  }
  const newMenu = await Menu.new({ items });

  // 先換上新選單，再關掉舊的，避免中間有一瞬間沒有選單
  const oldMenu = contextMenu;
  contextMenu = newMenu;
  if (oldMenu !== null) {
    await oldMenu.close();
  }
}

/** 在滑鼠位置（視窗內的 CSS 像素）彈出右鍵選單 */
export async function popupContextMenu(x: number, y: number): Promise<void> {
  if (!inTauri || contextMenu === null) {
    return;
  }
  await contextMenu.popup(new LogicalPosition(x, y));
}
