// =========================================================
// 檔案：main.ts
// 用途：建立 Vue 應用程式、載入 i18n、內嵌字型與共用／主題／設定視窗樣式。
//       字型不能從網路載入（程式不連網）：工程師主題的 JetBrains Mono、霓虹主題的 Orbitron
//       用 SIL OFL 授權的 @fontsource 套件打包進程式（只取拉丁字元集與用到的字重；授權見 THIRD_PARTY_NOTICES.md）。
//       中文與其他字型用 Windows 內建字型（43「字體排版」）。
// 誰會用到：index.html 在 WebView2 載入時執行。
// =========================================================

import { createApp } from "vue";
import App from "./App.vue";
import { i18n } from "./i18n";
import "@fontsource/jetbrains-mono/latin-400.css";
import "@fontsource/jetbrains-mono/latin-700.css";
import "@fontsource/orbitron/latin-500.css";
import "@fontsource/orbitron/latin-700.css";
import "./styles/base.css";
import "./styles/card.css";
import "./styles/layout.css";
import "./styles/settings.css";
import "./styles/settings-pages.css";
import "./themes/minimal.css";
import "./themes/glass.css";
import "./themes/terminal.css";
import "./themes/gauge.css";
import "./themes/neon.css";

createApp(App).use(i18n).mount("#app");
