# Just Keep Identity (jki)
> **Extreme speed MFA & Identity Session Manager for CLI Power Users.**

![JKI Demo](docs/assets/demo.gif)

[English](README.md) / [繁體中文](README.zh-TW.md)

`jki` 是一個專為工程師設計的身份授權工具。我們不只是要管理 TOTP，我們是要在不離開終端機的前提下，以極速完成身份驗證。

## 核心哲學 (Philosophy)

*   **流暢感 (Flow)**: 查詢與產碼反應極其迅速。
*   **Fuzzy Intelligence**: 支援模糊搜尋與匹配字元高亮顯示，即使記不清全名也能瞬間定位。
*   **Smart Agent**: 智慧背景代理，支援明文金庫自動解鎖與磁碟資料主動同步 (Active Reload)。
*   **物理隔離與安全**: 基於 OS Keyring，所有秘密鎖在系統的安全保險箱中，絕對拒絕雲端。
*   **人體工學 (Ergonomics)**: 專門優化的 Micro-Roll 指令集，單手即可完成產碼。
*   **分級分發策略**:
    *   **macOS**: 提供包含 GUI 托盤代理的完整套件，**經過官方簽名與公證**，確保最佳體驗。
    *   **Linux & Windows**: 提供輕量化 **純 CLI 核心**。高效、可攜，並直接與系統金鑰鍊整合。

## 技術架構 (Technical DNA)

`jki` 採用 Rust 構建，追求極致的穩定性與安全性：

*   **智慧型代理 (Intelligent Agent)**: `jki-agent` 持有解密後的記憶體快取。它具備 **自動 TTL 機制 (預設 1 小時)**，會在閒置後自動清理記憶體中的秘密。*(目前針對 macOS 深度優化)*。
*   **混合金庫 (Hybrid Vault)**:

    *   **元數據 (Metadata)**: 透過本地檔案管理，支援 Git 版本控制。
    *   **金鑰秘密 (Secrets)**: 直接與 OS 原生 Keyring (macOS Keychain, Linux Secret Service) 整合。
*   **Unix-Friendly**: 完美的管道支援 (`stdout -`)，輕鬆與 `ssh`, `git`, `kubectl` 等工具整合。

## 快速開始 (Quick Start)

```bash
# 查詢並複製 OTP (優先向 Agent 要，若無 Agent 則支援 master.key 或直接問密碼)
jki github

# 智慧過濾：搜尋 google 並直接選擇第 2 個結果
jki google 2

# 驗證過濾結果：列出搜尋結果而不執行
jki google 2 -l

# 快速同步金庫 (Git commit/pull/push)
jkim git sync
```

### 智慧過濾與選擇 (Smart Filtering & Selection)

`jki` 遵循「過濾 (Filter) -> 動作 (Action)」的邏輯鏈，讓你在複雜的帳號清單中如魚得水：

1.  **多重過濾**: `jki [PATTERNS]... [INDEX]`
    *   `jki u`：列出所有符合 `u` 的帳號 (如 Uber, Uplay)。
    *   `jki u 2`：直接獲取 `u` 搜尋結果中第 2 項的 OTP。
2.  **清單模式 (`-l, --list`)**: 
    *   任何時候加上 `-l` 都會將 `jki` 切換為「只列出、不執行」模式。
    *   這對於在大量結果中確認索引號 (`INDEX`) 非常有用。
3.  **無感報錯**: 搜尋結果不唯一時不再視為錯誤，而是優雅地列出候選清單並提示你如何精確選擇。

---

## 📦 安裝方式

### 方案 A：Homebrew (macOS 推薦)
```bash
brew tap lichih/jki
brew install jki
```

### 方案 B：源碼編譯 (開發者/Linux)
請確保你已安裝 [Rust 工具鏈](https://rustup.rs/)：
```bash
git clone https://github.com/lichih/just-keep-identity.git
cd just-keep-identity
make install

# 針對 Linux/Windows (無介面代理模式):
./install.sh --headless
```

---

## 🛡 安全架構與心理模型 (Security Architecture)

JKI 採用**「關注點分離」**策略，在確保最高安全性的同時不犧牲可移植性：

| 組件 | 儲存類型 | 內容 | 可移植性 |
| :--- | :--- | :--- | :--- |
| **身份元數據 (Metadata)** | Git / 本地檔案 | 帳號名稱、發行者、索引資訊 | **高** (透過 Git 同步) |
| **OTP 秘密 (Secrets)** | OS Keyring | 實際的 TOTP Secret Keys | **零** (鎖定在硬體設備) |

### 為什麼這樣設計？
- **零磁碟殘留**：你的實際金鑰永遠不會以明文形式寫入磁碟。它們被儲存在作業系統原生的保險箱中（如 macOS Keychain）。
- **安全的同步**：你可以放心地將 JKI 的 Git 儲存庫推送到私有雲端。即使儲存庫外洩，攻擊者也只能看到你有「哪些」帳號，而拿不到「進入」這些帳號的金鑰。
- **排除策略 (Exclusion Policy)**：JKI 預設的 `.gitignore` 會自動排除明文檔案 (`vault.json`, `master.key`, `*.txt`)。
- **自動加固同步 (Auto-Hardening Sync)**：當執行 `jkim git sync` 時，系統會主動偵測明文金鑰。若目前有可用的 Master Key（透過 Agent 或 Keychain），JKI 會**自動執行加密並替換明文檔案**，確保您的秘密在同步過程中始終受到 `age` 加密保護。

## 🔄 同步與災難恢復 (Sync & Disaster Recovery)

### 設定新電腦
1. 將 JKI 儲存庫 `git clone` 到新電腦。
2. 執行 `jkim git sync` 恢復你的帳號結構。
3. **重要**：你必須使用 `jkim add -f <account>` 為每個帳號手動重新輸入 Secret。元數據會隨 Git 遷移，但秘密不會。

### 災難恢復計畫
- **備份**：我們建議將原始的 2FA 恢復碼 (Recovery Codes) 妥善保存在離線位置（如物理保險箱）。
- **恢復**：如果你失去了對 OS Keyring 的存取權（例如電腦重灌且無備份），請使用恢復碼重設 2FA 並重新加入 JKI。

---

*Built with ❤️ for those who live in the terminal.*
