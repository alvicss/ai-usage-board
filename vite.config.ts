// =========================================================
// 檔案：vite.config.ts
// 用途：設定 Vue 開發伺服器與 Tauri 建置時使用的 Vite 行為。
// 誰會用到：npm run dev、npm run build 與 Tauri CLI。
// =========================================================

import process from "node:process";
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

const developmentHost = process.env.TAURI_DEV_HOST;

export default defineConfig(() => ({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: developmentHost || false,
    hmr: developmentHost
      ? {
          protocol: "ws",
          host: developmentHost,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
