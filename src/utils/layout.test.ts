// 檔案：utils/layout.test.ts
// 用途：驗證隨視窗大小調整的規則：縮放比例、並排橫排／直排、卡片寬高、精簡模式（三種版面），
//       工程師主題的字元條格數、儀表主題的環大小（43「響應式規則」「五種主題」）。
// 誰會用到：`npm run test`（Vitest）。

import { describe, expect, it } from "vitest";
import {
  BASE_FONT_PX,
  computeLayout,
  gaugeRingMaxHeight,
  monoTextWidthEm,
  needsCompact,
  terminalRowLayout,
} from "./layout";

describe("computeLayout", () => {
  it("預設大小時縮放比例為 1（與畫面示意一致）", () => {
    expect(computeLayout(360, 230, "carousel", 1).scale).toBeCloseTo(1, 5);
    expect(computeLayout(639, 240, "overview", 3).scale).toBeCloseTo(1, 5);
  });

  it("基準字級限制在 11～18px", () => {
    const tiny = computeLayout(240, 150, "carousel", 1);
    expect(tiny.scale * BASE_FONT_PX).toBeCloseTo(11, 5);
    const huge = computeLayout(1600, 1000, "carousel", 1);
    expect(huge.scale * BASE_FONT_PX).toBeCloseTo(18, 5);
  });

  it("並排：寬高比 ≥ 1.6 橫排，否則直排", () => {
    expect(computeLayout(640, 240, "overview", 3).direction).toBe("row");
    expect(computeLayout(360, 600, "overview", 3).direction).toBe("column");
  });

  it("直排時每張卡片的高度是平均分配", () => {
    const layout = computeLayout(360, 612, "overview", 3);
    // (612 - 外圍 12 - 卡片間距 20) ÷ 3
    expect(layout.cardHeight).toBeCloseTo((612 - 12 - 20) / 3, 5);
  });

  it("橫排時每張卡片的寬度是平均分配；輪播時是整個寬度扣掉外圍", () => {
    // (640 - 外圍 12 - 卡片間距 20) ÷ 3
    expect(computeLayout(640, 240, "overview", 3).cardWidth).toBeCloseTo((640 - 12 - 20) / 3, 5);
    expect(computeLayout(360, 230, "carousel", 1).cardWidth).toBe(348);
  });

  it("輪播高度低於 170 一律精簡", () => {
    expect(computeLayout(360, 160, "carousel", 1).forceCompact).toBe(true);
    expect(computeLayout(360, 230, "carousel", 1).forceCompact).toBe(false);
  });
});

describe("needsCompact", () => {
  const layout = computeLayout(360, 230, "carousel", 1);

  it("橫條：兩個額度在預設大小放得下完整版面，四個放不下", () => {
    expect(needsCompact(layout.cardHeight, 2, layout.scale, layout.forceCompact, "minimal")).toBe(false);
    expect(needsCompact(layout.cardHeight, 4, layout.scale, layout.forceCompact, "glass")).toBe(true);
  });

  it("字元條：重置時間在同一行，四個額度在預設大小也放得下", () => {
    expect(needsCompact(layout.cardHeight, 4, layout.scale, layout.forceCompact, "terminal")).toBe(false);
  });

  it("環狀表：額度多少不影響高度；卡片太矮才精簡", () => {
    expect(needsCompact(layout.cardHeight, 4, layout.scale, layout.forceCompact, "gauge")).toBe(false);
    expect(needsCompact(150, 2, 1, false, "gauge")).toBe(true);
  });

  it("已被強制精簡時一律精簡", () => {
    expect(needsCompact(999, 1, 1, true, "minimal")).toBe(true);
  });
});

describe("工程師主題的字元條格數", () => {
  it("中文算一個字寬、英數字算 0.6", () => {
    expect(monoTextWidthEm("5 小時")).toBeCloseTo(0.6 * 2 + 2, 5);
    expect(monoTextWidthEm("5h")).toBeCloseTo(1.2, 5);
  });

  const shortLabel = monoTextWidthEm("!5 小時");
  const longLabel = monoTextWidthEm("!Claude+GPT 5 小時");
  const resetEm = monoTextWidthEm("↻ 2h13m");

  it("預設大小、短標籤時約 20 格並顯示重置時間；最多 24 格", () => {
    const layout = terminalRowLayout(348, 1, shortLabel, resetEm);
    expect(layout.cells).toBeGreaterThanOrEqual(18);
    expect(layout.cells).toBeLessThanOrEqual(22);
    expect(layout.showReset).toBe(true);
    expect(terminalRowLayout(2000, 1, shortLabel, resetEm).cells).toBe(24);
  });

  it("標籤很長時字元條縮短（少於 10 格），仍顯示重置時間", () => {
    const layout = terminalRowLayout(348, 1, longLabel, resetEm);
    expect(layout.cells).toBeGreaterThanOrEqual(4);
    expect(layout.cells).toBeLessThan(10);
    expect(layout.showReset).toBe(true);
  });

  it("卡片很窄時重置時間改放提示，字元條 4～10 格", () => {
    const narrow = terminalRowLayout(206, 1, longLabel, resetEm);
    expect(narrow.showReset).toBe(false);
    expect(narrow.cells).toBe(4);
    const medium = terminalRowLayout(206, 1, shortLabel, resetEm);
    expect(medium.cells).toBeGreaterThanOrEqual(4);
    expect(medium.cells).toBeLessThanOrEqual(10);
  });
});

describe("儀表主題的環大小", () => {
  it("卡片越矮環越小，但不會小於下限", () => {
    const tall = gaugeRingMaxHeight(218, 1, false);
    const short = gaugeRingMaxHeight(150, 1, false);
    expect(tall).toBeGreaterThan(short);
    expect(gaugeRingMaxHeight(50, 1, false)).toBe(24);
  });

  it("精簡模式不顯示重置時間，環可以大一點", () => {
    expect(gaugeRingMaxHeight(180, 1, true)).toBeGreaterThan(gaugeRingMaxHeight(180, 1, false));
  });
});
