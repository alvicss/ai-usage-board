// 檔案：utils/appearance.ts
// 用途：外觀相關的純計算：
//       1. 色系：把設定的 auto／light／dark 換算成實際的亮或暗（霓虹固定暗色，43「色系」）。
//       2. 卡片邊框：依「目前主題＋色系」取出 FR-18 的邊框設定，換成要覆寫的 CSS 變數（43「邊框自訂」）。
//       3. 小工具根元素的 CSS 變數：縮放、警示色、不透明度、邊框覆寫（小工具與設定頁預覽共用）。
// 誰會用到：WidgetView（套在小工具根元素上）、設定畫面的即時預覽（WidgetPreview）。

import type { CardBorderSettings, ColorScheme, Settings, ThemeId } from "../types/settings";

/** 有外發光邊框的主題：自訂邊框色時，外發光也改成同色 */
const THEMES_WITH_BORDER_GLOW: ThemeId[] = ["neon"];

/**
 * 換算實際色系。
 * 輸入：主題、設定的色系（auto／light／dark）、Windows 目前是否為深色模式。
 */
export function resolveScheme(
  theme: ThemeId,
  colorScheme: "auto" | ColorScheme,
  systemIsDark: boolean,
): ColorScheme {
  // 霓虹只有暗色（發光效果要靠暗底）
  if (theme === "neon") {
    return "dark";
  }
  if (colorScheme === "auto") {
    return systemIsDark ? "dark" : "light";
  }
  return colorScheme;
}

/** 邊框模式：主題預設、自訂顏色、無邊框 */
export type BorderMode = "default" | "custom" | "none";

/** 邊框設定換算的結果：模式，以及要寫到根元素上的 CSS 變數 */
export interface BorderStyle {
  mode: BorderMode;
  variables: Record<string, string>;
}

/** 判斷文字是不是 #RRGGBB 格式的顏色 */
export function isHexColor(value: string): boolean {
  return /^#[0-9a-fA-F]{6}$/.test(value);
}

/** 取出某個「主題＋色系」的邊框設定值（例如 minimal_dark）；沒有這一組時視為 default */
export function cardBorderValue(
  card: CardBorderSettings,
  theme: ThemeId,
  scheme: ColorScheme,
): string {
  const key = `${theme}_${scheme}`;
  const value = (card as unknown as Record<string, string | undefined>)[key];
  if (value === undefined) {
    return "default";
  }
  return value;
}

/** 依設定換算卡片邊框要覆寫的 CSS 變數（--w-border、--w-border-glow） */
export function cardBorderStyle(settings: Settings, theme: ThemeId, scheme: ColorScheme): BorderStyle {
  const value = cardBorderValue(settings.borders.card, theme, scheme);

  if (value === "none") {
    return {
      mode: "none",
      variables: { "--w-border": "transparent", "--w-border-glow": "none" },
    };
  }

  if (isHexColor(value)) {
    const variables: Record<string, string> = { "--w-border": value };
    if (THEMES_WITH_BORDER_GLOW.includes(theme)) {
      variables["--w-border-glow"] = `0 0 12px ${value}`;
    }
    return { mode: "custom", variables };
  }

  return { mode: "default", variables: {} };
}

/**
 * 小工具根元素上的 CSS 變數（小工具與設定頁預覽共用同一套算法）。
 * 輸入：設定、實際色系、縮放比例。
 */
export function widgetRootStyle(settings: Settings, scheme: ColorScheme, scale: number): Record<string, string> {
  const style: Record<string, string> = {
    "--scale": scale.toFixed(4),
    // 警示色（外框、該行底色、圓點）一律取 --w-alert（FR-18）
    "--w-alert": settings.borders.alert_color,
    "--w-opacity": String(settings.display.opacity_percent / 100),
  };

  // 卡片邊框：依目前主題＋色系覆寫 --w-border／--w-border-glow（主題預設時不覆寫）
  const border = cardBorderStyle(settings, settings.display.theme, scheme);
  for (const [name, value] of Object.entries(border.variables)) {
    style[name] = value;
  }
  return style;
}
