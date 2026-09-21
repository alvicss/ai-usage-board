# AI Usage Board — 三家 AI 訂閱用量，一個桌面小工具

> 本檔是 AI Usage Board 給人看的第一份文件：它是什麼、怎麼安裝使用、怎麼完全移除、常見問題。
> **目前狀態**：v1.0 已完成；發布檔請見本專案 GitHub Releases。

## 系統簡介

AI Usage Board 是一個 Windows 桌面小工具，把 **Claude**、**Codex**、**Antigravity** 三個 AI 訂閱的用量整合在一起，像天氣小工具一樣釘在桌面上，瞄一眼就知道每家還剩多少、什麼時候重置。

- **三家一起看**：輪播（一次一家）或並排（同時比較），按一下切換；快用完時卡片會泛紅光提醒。
- **只請官方 CLI 自己回報**：不讀取任何登入憑證、不呼叫未公開的 API，查詢用量**不消耗任何額度**。
- **好看、可以自己調**：玻璃（Windows 11 的毛玻璃材質）、簡約、工程師、儀表、霓虹五種主題，除了霓虹都有亮暗兩版（可跟隨 Windows 淺色／深色）；可以調整體不透明度、自訂進度條與邊框的顏色。
- **常駐不打擾**：背景自動更新（預設 10 分鐘，可調），也可以按 ⟳ 立即更新；查詢時不跳出黑色視窗；可設定開機自動啟動。
- **繁體中文與英文**：預設自動選——Windows 顯示語言是中文，或 Windows 地區設在台灣、香港、澳門時用繁體中文，其他用英文；也可以在設定手動切換，立即生效。

畫面長什麼樣子，請用瀏覽器打開 [docs/design/ui-mockup.html](docs/design/ui-mockup.html)（畫面示意，數字是示範資料）。

## 快速開始

### 使用者

**需要的環境**

- Windows 11（Windows 10 也可以用，但沒有玻璃模糊效果）。
- 至少安裝並登入下列其中一家的官方 CLI（沒訂閱的那家可以在設定裡關掉）：
  - Claude：Claude Code（`claude`）
  - Codex：Codex CLI（`codex`）
  - Antigravity：Antigravity CLI（`agy`，需要 1.1.11 以上版本）

**安裝與啟動**

1. 到本專案 GitHub 的 Releases（https://github.com/alvicss/ai-usage-board/releases ）下載 `AI-Usage-Board-v1.0-portable.exe`（免安裝版），放在任何資料夾都可以。
2. 第一次執行時，Windows 可能跳出藍色的「Windows 已保護您的電腦」。這是因為本程式沒有購買數位簽章憑證（免費開源專案），不是病毒警告。請按「**其他資訊**」→「**仍要執行**」。
   - 想確認檔案沒被竄改：在 PowerShell 執行 `Get-FileHash .\AI-Usage-Board-v1.0-portable.exe -Algorithm SHA256`，跟 Release 頁面上的 SHA-256 比對。
3. 小工具會出現在主螢幕右上角，釘在桌面上（在其他視窗下方，不佔工作列）。可以拖曳、拉邊框調整大小，下次開啟會記住位置和大小；輪播、並排兩種顯示方式各自記住大小。已經開著時再執行一次，不會多開一個，現有的小工具會浮上來閃一下，讓你找到它。按 Win+D（顯示桌面）時小工具會暫時浮上來，切回其他視窗後自動回到桌面最下層。
4. 小工具上的三顆按鈕：⟳ 立即更新、⇄ 切換輪播／並排、⚙ 設定。在小工具上按右鍵，還有「關於」與「結束」；關於頁有 GitHub 連結，可以看新版與回報問題。
5. Windows 右下角通知區（時鐘旁邊，可能收在「^」裡）也有本程式的圖示：右鍵有同樣的選單（含「結束」），左鍵按一下小工具會浮上來閃一下，找不到小工具時很好用。

**完全移除**

1. 在小工具上按右鍵 →「設定」，關閉「開機自動啟動」。
2. 在小工具或右下角通知區的圖示上按右鍵 →「結束」。
3. 刪除 exe 檔。
4. 刪除這兩個資料夾（在檔案總管的網址列貼上即可打開）：
   - `%APPDATA%\AI Usage Board`（設定、快取、記錄檔；舊版留下的背景圖資料夾也在裡面）
   - `%LOCALAPPDATA%\com.halfmemo.aiusageboard`（畫面引擎的暫存資料）

### 開發者

- 需要：Node.js、Rust（`cargo`）、Microsoft C++ Build Tools、WebView2（Windows 11 內建）、PowerShell 7（文件檢查用）。
- 技術：Tauri 2（Rust）＋ Vue 3 ＋ TypeScript。
- 常用指令、文件導覽、開發規範：先看 [CLAUDE.md](CLAUDE.md)，目前進度看 [docs/00_STATUS.md](docs/00_STATUS.md)；打包與發布步驟看 [docs/80_DEPLOYMENT.md](docs/80_DEPLOYMENT.md)。
- ⚠ 手動測試 `agy` 時**不能用 Git Bash**（它會把 `/usage` 改寫成路徑，agy 會把它當成問題去問模型、真的消耗額度），請用 PowerShell。詳見 [docs/22_CONSTRAINTS.md](docs/22_CONSTRAINTS.md)。

## 常見問題

**Q：卡片顯示「找不到 Claude／Codex／agy」？**
A：確認有安裝該 CLI、而且在終端機打指令能執行。裝在特別位置的話，到「設定 → 服務」填入 CLI 程式檔的位置。沒有訂閱這家，直接在同一頁關掉它。

**Q：卡片顯示「尚未登入」？**
A：先在終端機（PowerShell）執行一次該 CLI 並完成登入，再按 ⟳。

**Q：Antigravity 顯示「偵測到異常，已暫停查詢」？**
A：這是保護機制：程式偵測到 agy 可能把查詢當成一般問題回答（可能消耗額度），所以先停下來。請把 agy 更新到最新版，確認沒有用 Git Bash 啟動本程式，再到「設定 → 服務」按「我已確認，恢復查詢」。

**Q：數字跟官方 `/usage` 畫面不一樣？**
A：小工具預設每 10 分鐘更新一次，按 ⟳ 可以立即更新；卡片左下角會顯示資料是幾分鐘前的。

**Q：會不會讀我的帳號密碼？會不會消耗額度？**
A：不會。本程式只請各家官方 CLI 自己回報用量，不讀取任何登入憑證、本身也不連網；查詢用量不會呼叫 AI 模型。

**Q：介面語言不是我要的？**
A：到「設定 → 一般 → 介面語言」選繁體中文或 English，按儲存就立即換。「自動」會依 Windows 顯示語言與地區決定（見上方「系統簡介」）。

**Q：玻璃主題看起來是一片灰、或沒有毛玻璃？**
A：毛玻璃材質需要 Windows 11（22H2 以後）；Windows 10 上玻璃主題會改成半透明底，這是正常的。

**Q：出問題要看記錄檔？**
A：記錄檔在 `%APPDATA%\AI Usage Board\logs\`，每天一個檔，保留 7 天（設定可調）。記錄檔不含 email 或帳號資訊，可以附在問題回報裡。

## 文件索引

詳見 [CLAUDE.md](CLAUDE.md)。

## 授權

以 MIT 授權開源，全文見 [LICENSE](LICENSE)。作者／出處：**半桶水的AI筆記**（https://halfmemo.com/ ）。

程式裡內嵌兩套 SIL Open Font License 1.1 授權的字型：JetBrains Mono（工程師主題）、Orbitron（霓虹主題），版權聲明與授權全文見 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。其他字型用 Windows 內建字型。

## 支援

有問題或建議，請到本專案 GitHub 的 Issues 回報：https://github.com/alvicss/ai-usage-board/issues 。回報時請附上：Windows 版本、關於頁的版本號、有問題的是哪一家、記錄檔內容。新版本會放在 https://github.com/alvicss/ai-usage-board/releases （程式不會自動更新）。

## English

AI Usage Board is a Windows desktop widget that shows your **Claude**, **Codex** and **Antigravity** subscription usage in one place — as a carousel or side by side — with five themes, adjustable opacity and colors, and English / Traditional Chinese UI. It only asks each vendor's **official CLI** to report usage: it never reads login credentials, never calls undocumented APIs, and checking usage consumes no quota. The language is picked automatically (Traditional Chinese when the Windows display language is Chinese or the region is Taiwan, Hong Kong or Macao; English otherwise) and can be changed in Settings. Download: https://github.com/alvicss/ai-usage-board/releases (portable exe, no installer; unsigned, so Windows SmartScreen may warn on first run: click "More info" → "Run anyway"). Issues: https://github.com/alvicss/ai-usage-board/issues. License: MIT; bundled fonts JetBrains Mono and Orbitron are under the SIL Open Font License 1.1 (see THIRD_PARTY_NOTICES.md).
