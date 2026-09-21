// 檔案：utils/color.ts
// 用途：依已用 % 計算「綠 → 橘 → 紅」的線性漸變色（依 43「色彩系統」）。
//       整條進度條同一色；百分比數字也用這個顏色，亮色系時再與黑色混合 15%，白底上才看得清楚。
// 誰會用到：QuotaRow（進度條與百分比數字）。

import type { ColorScheme, ColorSettings } from "../types/settings";

/** 把 "#RRGGBB" 拆成 [紅, 綠, 藍] 三個 0～255 的數字 */
export function hexToRgb(hex: string): [number, number, number] {
  const value = hex.replace("#", "");
  const red = Number.parseInt(value.slice(0, 2), 16);
  const green = Number.parseInt(value.slice(2, 4), 16);
  const blue = Number.parseInt(value.slice(4, 6), 16);
  return [red, green, blue];
}

/**
 * 計算已用 % 對應的顏色（三個數字）。
 * 做法：已用 % 落在「綠→橘」或「橘→紅」哪一段，就依比例線性混合那一段頭尾兩色。
 */
export function usageRgb(usedPercent: number, colors: ColorSettings): [number, number, number] {
  // 先把已用 % 限制在 0～100
  let percent = usedPercent;
  if (percent < 0) {
    percent = 0;
  }
  if (percent > 100) {
    percent = 100;
  }

  const middle = colors.mid_position;
  let fromColor: [number, number, number];
  let toColor: [number, number, number];
  let step: number;
  let span: number;

  if (percent <= middle) {
    // 前半段：綠 → 橘
    fromColor = hexToRgb(colors.low);
    toColor = hexToRgb(colors.mid);
    step = percent;
    span = middle;
  } else {
    // 後半段：橘 → 紅
    fromColor = hexToRgb(colors.mid);
    toColor = hexToRgb(colors.high);
    step = percent - middle;
    span = 100 - middle;
  }
  if (span < 1) {
    span = 1;
  }

  const mixed: [number, number, number] = [0, 0, 0];
  for (let index = 0; index < 3; index++) {
    mixed[index] = Math.round((fromColor[index] * (span - step) + toColor[index] * step) / span);
  }
  return mixed;
}

/** 進度條用的顏色，例如 "rgb(1, 180, 104)" */
export function usageColor(usedPercent: number, colors: ColorSettings): string {
  const [red, green, blue] = usageRgb(usedPercent, colors);
  return `rgb(${red}, ${green}, ${blue})`;
}

/** 百分比數字用的顏色：暗色系與進度條相同；亮色系與黑色混合 15%（43「色系」可讀性調整） */
export function numberColor(usedPercent: number, colors: ColorSettings, scheme: ColorScheme): string {
  const [red, green, blue] = usageRgb(usedPercent, colors);
  if (scheme !== "light") {
    return `rgb(${red}, ${green}, ${blue})`;
  }
  const darkRed = Math.round(red * 0.85);
  const darkGreen = Math.round(green * 0.85);
  const darkBlue = Math.round(blue * 0.85);
  return `rgb(${darkRed}, ${darkGreen}, ${darkBlue})`;
}
