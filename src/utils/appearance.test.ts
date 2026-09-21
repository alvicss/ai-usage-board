// 檔案：utils/appearance.test.ts
// 用途：驗證色系換算（FR-8：跟隨 Windows、霓虹固定暗色）、卡片邊框覆寫規則（FR-18）、根元素 CSS 變數。
// 誰會用到：`npm run test`（Vitest）。

import { describe, expect, it } from "vitest";
import { previewSettingsResponse } from "../services/preview-data";
import { cardBorderStyle, isHexColor, resolveScheme, widgetRootStyle } from "./appearance";

describe("resolveScheme", () => {
  it("跟隨 Windows 時依系統深淺色", () => {
    expect(resolveScheme("minimal", "auto", true)).toBe("dark");
    expect(resolveScheme("minimal", "auto", false)).toBe("light");
  });

  it("指定亮或暗時照設定", () => {
    expect(resolveScheme("glass", "light", true)).toBe("light");
    expect(resolveScheme("glass", "dark", false)).toBe("dark");
  });

  it("霓虹不受色系影響，永遠暗色", () => {
    expect(resolveScheme("neon", "light", false)).toBe("dark");
    expect(resolveScheme("neon", "auto", false)).toBe("dark");
  });
});

describe("cardBorderStyle", () => {
  it("主題預設：不覆寫任何變數", () => {
    const settings = previewSettingsResponse().settings;
    expect(cardBorderStyle(settings, "minimal", "dark")).toEqual({ mode: "default", variables: {} });
  });

  it("無邊框：邊框透明、外發光關閉", () => {
    const settings = previewSettingsResponse().settings;
    settings.borders.card.minimal_light = "none";
    expect(cardBorderStyle(settings, "minimal", "light")).toEqual({
      mode: "none",
      variables: { "--w-border": "transparent", "--w-border-glow": "none" },
    });
  });

  it("自訂顏色：只換邊框；有外發光的主題（霓虹）外發光也換成同色", () => {
    const settings = previewSettingsResponse().settings;
    settings.borders.card.minimal_dark = "#123456";
    settings.borders.card.neon_dark = "#22E4FF";
    expect(cardBorderStyle(settings, "minimal", "dark").variables).toEqual({ "--w-border": "#123456" });
    expect(cardBorderStyle(settings, "neon", "dark").variables).toEqual({
      "--w-border": "#22E4FF",
      "--w-border-glow": "0 0 12px #22E4FF",
    });
  });

  it("各主題＋色系各自記住", () => {
    const settings = previewSettingsResponse().settings;
    settings.borders.card.minimal_dark = "none";
    expect(cardBorderStyle(settings, "minimal", "light").mode).toBe("default");
    expect(cardBorderStyle(settings, "glass", "dark").mode).toBe("default");
  });

  it("只接受 #RRGGBB", () => {
    expect(isHexColor("#FF3B3B")).toBe(true);
    expect(isHexColor("#FFF")).toBe(false);
    expect(isHexColor("red")).toBe(false);
  });
});

describe("widgetRootStyle", () => {
  it("警示色、縮放、不透明度換成 CSS 變數；主題預設邊框不覆寫", () => {
    const settings = previewSettingsResponse().settings;
    settings.borders.alert_color = "#00AAFF";
    settings.display.opacity_percent = 80;
    const style = widgetRootStyle(settings, "dark", 1.25);
    expect(style["--w-alert"]).toBe("#00AAFF");
    expect(style["--scale"]).toBe("1.2500");
    expect(style["--w-opacity"]).toBe("0.8");
    expect(style["--w-border"]).toBeUndefined();
  });

  it("目前主題＋色系設了自訂邊框或無邊框時，覆寫邊框變數", () => {
    const settings = previewSettingsResponse().settings;
    settings.display.theme = "minimal";
    settings.borders.card.minimal_light = "#223344";
    settings.borders.card.minimal_dark = "none";
    expect(widgetRootStyle(settings, "light", 1)["--w-border"]).toBe("#223344");
    const darkStyle = widgetRootStyle(settings, "dark", 1);
    expect(darkStyle["--w-border"]).toBe("transparent");
    expect(darkStyle["--w-border-glow"]).toBe("none");
  });
});
