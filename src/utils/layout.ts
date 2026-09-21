// 檔案：utils/layout.ts
// 用途：小工具「隨視窗大小調整」的純計算（43「響應式規則」）：
//       1. 基準字級與縮放比例：所有尺寸以 43 的 px 值乘上縮放比例。
//       2. 並排模式橫排或直排：寬高比 ≥ 1.6 橫排，否則直排；每張卡片的可用寬高。
//       3. 精簡模式：卡片高度放不下完整內容時，隱藏重置時間（改用滑鼠停留提示）；三種版面（橫條、字元條、環狀表）各自估算。
//       4. 工程師主題的字元進度條格數（一般 10～24 格，依卡片寬度；卡片很窄時最少 4 格、重置時間改放提示）、
//          儀表主題的環最大高度。
// 誰會用到：WidgetView（算縮放與排列方向）、ProviderCard（判斷自己要不要精簡、字元條格數、環的大小）。

import type { DisplayMode, ThemeId } from "../types/settings";

/** 預設大小（輪播 360×230）時的基準字級：360 ÷ 28 ≈ 12.86px，43 的 px 值都以這個大小為準 */
export const BASE_FONT_PX = 360 / 28;
/** 基準字級的上下限（43：clamp(11px, …, 18px)） */
const MIN_FONT_PX = 11;
const MAX_FONT_PX = 18;
/** 並排模式寬高比 ≥ 這個值時橫排 */
const ROW_ASPECT_RATIO = 1.6;
/** 輪播高度低於這個值一律精簡（43） */
const COMPACT_HEIGHT_PX = 170;
/** 參考尺寸：在這個大小時縮放比例為 1（與畫面示意一致） */
const CAROUSEL_REFERENCE = { width: 360, height: 230 };
const OVERVIEW_CARD_WIDTH = 213; // 640 寬放三張卡片時，每張約 213px
const OVERVIEW_ROW_HEIGHT = 240;
const OVERVIEW_COLUMN_CARD_HEIGHT = 190;
/** 小工具外圍保留的透明邊（base.css 的 .widget-shell padding），以及並排卡片間距 */
const SHELL_PADDING_PX = 6;
const CARD_GAP_PX = 10;
/** 卡片左右內距（card.css：16px） */
const CARD_PADDING_X_PX = 16;

/** 估算卡片高度用（px，縮放前）：標題列＋頁尾＋內距＋間距，以及一行額度（或一組環）完整顯示時的高度 */
const LAYOUT_METRICS = {
  /** 橫條（玻璃、簡約、霓虹）：一行額度＋下一行重置時間 */
  bars: { chrome: 84, fullRow: 55 },
  /** 字元條（工程師）：上面多一行提示字元；重置時間在同一行右邊 */
  terminal: { chrome: 100, fullRow: 24 },
};
/** 儀表：所有環排成一列；環下方的標籤（最多兩行）＋重置時間，以及環最小要多大才顯示重置時間 */
const GAUGE_CHROME_PX = 84;
const GAUGE_TEXT_FULL_PX = 44;
const GAUGE_TEXT_COMPACT_PX = 30;
const GAUGE_MIN_RING_PX = 56;
const GAUGE_SMALLEST_RING_PX = 24;

/** 工程師主題：額度列字級 12.5px、等寬字每個字寬約 0.6 字；格數限制 10～24 格（43） */
const TERMINAL_FONT_PX = 12.5;
const MONO_CHAR_EM = 0.6;
const TERMINAL_MIN_CELLS = 10;
const TERMINAL_MAX_CELLS = 24;
/** 卡片很窄時字元條最少幾格（標籤優先完整顯示） */
const TERMINAL_NARROW_MIN_CELLS = 4;
/** 工程師額度列：百分比欄約 4 字寬（字級 14px）、欄與欄之間 3 個 8px 間距、左右各 6px 列內距 */
const TERMINAL_PERCENT_PX = 4 * MONO_CHAR_EM * 14;
const TERMINAL_GAPS_PX = 3 * 8 + 12;

export type OverviewDirection = "row" | "column";

/** 小工具目前的排版結果 */
export interface WidgetLayout {
  /** 縮放比例（套到 CSS 的 --scale） */
  scale: number;
  /** 並排模式的排列方向（輪播模式固定為 row，不影響畫面） */
  direction: OverviewDirection;
  /** 每張卡片可用的寬度（px），給工程師主題算字元條格數 */
  cardWidth: number;
  /** 每張卡片可用的高度（px），給卡片判斷要不要精簡 */
  cardHeight: number;
  /** 視窗太矮，所有卡片一律精簡 */
  forceCompact: boolean;
}

/** 把數字限制在最小值與最大值之間 */
function clamp(value: number, minimum: number, maximum: number): number {
  if (value < minimum) {
    return minimum;
  }
  if (value > maximum) {
    return maximum;
  }
  return value;
}

/**
 * 依視窗大小、顯示方式與卡片數量算出排版。
 * 輸入：視窗內部寬高（px）、顯示方式、要顯示幾張卡片。
 */
export function computeLayout(
  width: number,
  height: number,
  mode: DisplayMode,
  cardCount: number,
): WidgetLayout {
  const cards = Math.max(1, cardCount);
  const innerWidth = Math.max(0, width - SHELL_PADDING_PX * 2);
  const innerHeight = Math.max(0, height - SHELL_PADDING_PX * 2);

  let direction: OverviewDirection = "row";
  let referenceWidth = CAROUSEL_REFERENCE.width;
  let referenceHeight = CAROUSEL_REFERENCE.height;
  let cardWidth = innerWidth;
  let cardHeight = innerHeight;

  if (mode === "overview") {
    const aspectRatio = width / Math.max(1, height);
    if (aspectRatio >= ROW_ASPECT_RATIO) {
      direction = "row";
      referenceWidth = OVERVIEW_CARD_WIDTH * cards;
      referenceHeight = OVERVIEW_ROW_HEIGHT;
      cardWidth = (innerWidth - CARD_GAP_PX * (cards - 1)) / cards;
    } else {
      direction = "column";
      referenceWidth = CAROUSEL_REFERENCE.width;
      referenceHeight = OVERVIEW_COLUMN_CARD_HEIGHT * cards;
      cardHeight = (innerHeight - CARD_GAP_PX * (cards - 1)) / cards;
    }
  }

  // 寬、高各自的放大倍數取小的那個，內容才不會超出視窗
  const rawScale = Math.min(width / referenceWidth, height / referenceHeight);
  const fontPx = clamp(BASE_FONT_PX * rawScale, MIN_FONT_PX, MAX_FONT_PX);
  const scale = fontPx / BASE_FONT_PX;

  const forceCompact = cardHeight < COMPACT_HEIGHT_PX - SHELL_PADDING_PX * 2;
  return { scale, direction, cardWidth, cardHeight, forceCompact };
}

/**
 * 判斷一張卡片放不放得下完整版面（每個額度都顯示重置時間）。
 * 輸入：卡片可用高度（px）、這張卡片要顯示幾個額度、縮放比例、是否已被強制精簡、目前主題。
 */
export function needsCompact(
  cardHeight: number,
  rowCount: number,
  scale: number,
  forceCompact: boolean,
  theme: ThemeId,
): boolean {
  if (forceCompact) {
    return true;
  }

  // 儀表：環排成一列，額度多少不影響高度；環小到放不下重置時間才精簡
  if (theme === "gauge") {
    const neededHeight = (GAUGE_CHROME_PX + GAUGE_TEXT_FULL_PX + GAUGE_MIN_RING_PX) * scale;
    return cardHeight < neededHeight;
  }

  let metrics = LAYOUT_METRICS.bars;
  if (theme === "terminal") {
    metrics = LAYOUT_METRICS.terminal;
  }
  const neededHeight = (metrics.chrome + metrics.fullRow * rowCount) * scale;
  return cardHeight < neededHeight;
}

/**
 * 儀表主題：環最多可以多高（px），環的大小依卡片高度自動縮小，下方的標籤與重置時間才放得下。
 * 輸入：卡片可用高度、縮放比例、是否精簡（精簡時不顯示重置時間，環可以大一點）。
 */
export function gaugeRingMaxHeight(cardHeight: number, scale: number, compact: boolean): number {
  let textHeight = GAUGE_TEXT_FULL_PX;
  if (compact) {
    textHeight = GAUGE_TEXT_COMPACT_PX;
  }
  const available = cardHeight - (GAUGE_CHROME_PX + textHeight) * scale;
  return Math.max(GAUGE_SMALLEST_RING_PX * scale, available);
}

/**
 * 估算一段文字用等寬字顯示時有幾個「字寬」（em）：中日韓全形字算 1，其他半形字算 0.6。
 * 工程師主題用來估算標籤與重置時間要佔多寬。
 */
export function monoTextWidthEm(text: string): number {
  let width = 0;
  for (const character of text) {
    const code = character.codePointAt(0) ?? 0;
    // U+2E80 之後大多是中日韓全形字
    if (code >= 0x2e80) {
      width += 1;
    } else {
      width += MONO_CHAR_EM;
    }
  }
  return width;
}

/** 工程師主題的額度列排法：字元條幾格、重置時間要不要顯示在行尾 */
export interface TerminalRowLayout {
  cells: number;
  showReset: boolean;
}

/**
 * 工程師主題的字元進度條要幾格、行尾放不放得下重置時間（43：一般是 10～24 格，依可用寬度）。
 * 輸入：卡片寬度（px）、縮放比例、最長的標籤與重置時間各佔幾個字寬（monoTextWidthEm 算出）。
 * 做法：卡片寬度扣掉內距、標籤、百分比與間距，剩下的空間給字元條與重置時間；標籤優先完整顯示：
 *   1. 放得下 10 格以上＋重置時間：照 43 的 10～24 格；
 *   2. 卡片較窄（例如 Antigravity 標籤長）：字元條縮到能放得下重置時間的格數，最少 4 格；
 *   3. 更窄（例如並排三張）：重置時間改放滑鼠停留提示，字元條最多 10 格、最少 4 格。
 */
export function terminalRowLayout(
  cardWidth: number,
  scale: number,
  labelEm: number,
  resetEm: number,
): TerminalRowLayout {
  const fontPx = TERMINAL_FONT_PX * scale;
  const cellPx = MONO_CHAR_EM * fontPx;
  const fixedWidth =
    CARD_PADDING_X_PX * 2 * scale + labelEm * fontPx + (TERMINAL_PERCENT_PX + TERMINAL_GAPS_PX) * scale;
  const available = cardWidth - fixedWidth;

  // 1、2：連同重置時間一起放
  const cellsWithReset = Math.floor((available - resetEm * fontPx) / cellPx);
  if (cellsWithReset >= TERMINAL_MIN_CELLS) {
    return { cells: Math.min(cellsWithReset, TERMINAL_MAX_CELLS), showReset: true };
  }
  if (cellsWithReset >= TERMINAL_NARROW_MIN_CELLS) {
    return { cells: cellsWithReset, showReset: true };
  }

  // 3：重置時間改放滑鼠停留提示
  const cellsWithoutReset = Math.floor(available / cellPx);
  return { cells: clamp(cellsWithoutReset, TERMINAL_NARROW_MIN_CELLS, TERMINAL_MIN_CELLS), showReset: false };
}
