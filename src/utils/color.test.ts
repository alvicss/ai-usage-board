// 檔案：utils/color.test.ts
// 用途：驗證「綠 → 橘 → 紅」漸變色的計算（FR-13 驗收：漸變數值正確）與亮色系數字加深。
// 誰會用到：`npm run test`（Vitest）。

import { describe, expect, it } from "vitest";
import type { ColorSettings } from "../types/settings";
import { hexToRgb, numberColor, usageColor, usageRgb } from "./color";

const defaults: ColorSettings = { low: "#01B468", mid: "#EA7500", high: "#FF0000", mid_position: 60 };

describe("hexToRgb", () => {
  it("把 #RRGGBB 拆成三個數字", () => {
    expect(hexToRgb("#01B468")).toEqual([1, 180, 104]);
    expect(hexToRgb("#ff0000")).toEqual([255, 0, 0]);
  });
});

describe("usageRgb／usageColor", () => {
  it("三個節點剛好是設定的三個顏色", () => {
    expect(usageRgb(0, defaults)).toEqual([1, 180, 104]);
    expect(usageRgb(60, defaults)).toEqual([234, 117, 0]);
    expect(usageRgb(100, defaults)).toEqual([255, 0, 0]);
  });

  it("節點之間線性混合", () => {
    // 30% 是綠→橘的正中間：(1+234)/2=117.5→118、(180+117)/2=148.5→149、(104+0)/2=52
    expect(usageRgb(30, defaults)).toEqual([118, 149, 52]);
    // 80% 是橘→紅的正中間：(234+255)/2=244.5→245、(117+0)/2=58.5→59、0
    expect(usageRgb(80, defaults)).toEqual([245, 59, 0]);
  });

  it("超出 0～100 時限制在兩端", () => {
    expect(usageRgb(-5, defaults)).toEqual([1, 180, 104]);
    expect(usageRgb(130, defaults)).toEqual([255, 0, 0]);
  });

  it("節點位置可以調整", () => {
    const custom: ColorSettings = { ...defaults, mid_position: 20 };
    expect(usageRgb(20, custom)).toEqual([234, 117, 0]);
  });

  it("輸出 CSS 可用的 rgb() 文字", () => {
    expect(usageColor(0, defaults)).toBe("rgb(1, 180, 104)");
  });
});

describe("numberColor", () => {
  it("暗色與進度條同色，亮色與黑色混合 15%", () => {
    expect(numberColor(100, defaults, "dark")).toBe("rgb(255, 0, 0)");
    expect(numberColor(100, defaults, "light")).toBe("rgb(217, 0, 0)");
    expect(numberColor(0, defaults, "light")).toBe("rgb(1, 153, 88)");
  });
});
