# Mission: JKI Onboarding & App Store Excellence

## 🎯 目標 (Goals)
在不改變 JKI 「CLI 為核心」的前提下，為 macOS 使用者（特別是透過 App Store 安裝的使用者）提供「開箱即用」的專業初始化體驗。將「付費購買」轉化為「購買省心與自動化」。

---

## 🛠 核心功能模組 (Core Components)

### 1. 極簡安裝助手 (The Minimalist Setup Assistant)
*   **定位**：一個輕量級的 macOS 原生視窗（Swift/SwiftUI），僅在第一次啟動或手動觸發時出現。
*   **功能**：
    *   **PATH 自動連結**：一鍵將 `jki` 執行檔連結到 `/usr/local/bin`，解決 MAS 沙盒下的路徑存取問題。
    *   **環境檢查**：自動偵測終端機環境（Zsh/Bash），並提示是否需要加入必要的 Alias 或 Auto-completion。

### 2. JKI Agent 自動化管理
*   **自動掛載**：提供「登入時自動啟動 Agent」的開關，自動處理 `LaunchAgents` 腳本的生成與放置。
*   **狀態監控**：在小視窗中顯示當前 Agent 的狀態（Running/Idle）以及 Master Key 的 TTL 剩餘時間。

### 3. 「誠實」的導引流程 (Honest Onboarding)
*   **快速示範**：視窗內展示 3 個核心指令的動畫或範例。
*   **權限授權導引**：引導使用者完成第一次 Keychain 的存取授權，避免使用者在終端機遇到突如其來的 OS 彈窗而感到困惑。

---

## 📐 實作策略 (Implementation Strategy)

### 「殼」與「核心」的分離
*   **Core (Rust)**：保持不變。所有的邏輯、安全加密、Keychain 交互依然由 Rust 核心處理。
*   **Wrapper (Swift/App Store)**：
    *   作為 MAS 的進入點。
    *   負責處理 macOS 特有的權限請求、LaunchAgents 管理與視覺引導。
    *   不包含任何業務邏輯，僅作為「配置工具」。

### Homebrew vs. App Store
*   **Homebrew**：維持現狀。使用者享有最高的自由度，手動設定環境，符合 Power User 習慣。
*   **App Store**：收費版。提供上述的「自動化配置助手」，滿足「付費買時間與省心」的使用者。

---

## 📝 任務清單 (Task Checklist)
- [ ] **設計 UI 原型**：極簡風格，不超過 3 個分頁。
- [ ] **實作 PATH 連結邏輯**：處理沙盒權限（Scoped Bookmarks）與路徑寫入。
- [ ] **整合 LaunchAgents**：確保 Agent 啟動邏輯在商店版與開源版之間保持一致。
- [ ] **撰寫「商店版合理化」聲明**：在 README 誠實交代為什麼商店版收費（為了分發、更新與自動化工具的維護）。

---

## 📈 成功指標 (Success Metrics)
- 使用者從按下「下載」到完成第一次 `jki add` 的時間縮短至 1 分鐘內。
- 零手動路徑設定（Zero-manual-PATH-config）。
- 獲得類似 WinSCP 的評價：雖然有免費版，但商店版「值得這個價」。

**狀態：規劃中 (Planning)**
