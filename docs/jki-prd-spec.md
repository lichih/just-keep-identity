# **Just Keep Identity (jki)：JK Suite 極速 MFA 數位金庫**

## **產品需求 (PRD) 與技術規格 (Spec) 文件 - V23 (交互控制與強制選項同步版)**

### **第一章：品牌與體系定義 (Brand & System)**

#### **1.1 品牌命名決策**
*   **正式名稱**：**Just Keep Identity (jki)**。
*   **人體工學**：主打 **右手單手操作 (Micro-Roll)**，支援本地 (Desktop) 與遠端 (SSH/Headless) 環境。

#### **1.2 平台支援矩陣 (Support Matrix)**
*   **Tier 1 (Desktop)**：macOS, Windows。支援生體辨識 (TouchID / Hello ESS)。
*   **Tier 2 (Headless / SSH)**：支援 0600 金鑰檔案、環境變數與互動式密碼輸入。

### ---

**第二章：交互邏輯與 Unix 工具鏈規範 (CLI Standards)**

#### **2.1 查詢行為與一致性 (Consistency)**
*   **單一結果**：複製 OTP / 輸出 stdout。
*   **清單模式 (預設)**：僅顯示 Metadata，**不計算 OTP** 以確保極速與安全。
*   **不一致處理 (Missing Secrets)**：
    *   **預設行為**：偵測到 Metadata 有帳號但加密庫無 Secret 時，印出 Warning 並列出受影響項目。
    *   **安靜模式 (-q)**：自動過濾遺失密鑰的帳號，不顯示 Warning，僅從搜尋池中移除。

#### **2.2 全域控制參數 (Global Flags)**
*   `-q / --quiet`: 安靜模式 (抑制 stderr 提示與一致性警告)。
*   `-I / --interactive`: **強制互動模式**。忽略磁碟上的 `master.key` 檔案與 Agent 快取，強制要求使用者手動輸入 Master Key。
*   `-`: stdout 模式 (純淨輸出 OTP)。
*   `--list`: 強制顯示匹配清單。
*   `-o / --otp`: 在清單模式下強制計算並顯示 OTP。
*   `--`: 終止選項解析。

### ---

**第三章：管理與自動化 (Management & Automation)**

#### **3.1 編輯模式 (jkim edit)**
*   採 Unix 原生的 `crontab -e` 哲學：
    *   **安全性**：於系統暫存目錄建立 0600 權限之 `.tmp.json` 檔案。
    *   **工具相容性**：呼叫 `$EDITOR` 進行編輯。
    *   **防呆機制**：編輯後強制執行 JSON Schema 驗證。

#### **3.2 金鑰管理 (jkim master-key)**
*   **指令集**：`set`, `remove`, `change`。
*   **強制選項 (--force / -f)**：
    *   適用於 `set` 與 `remove`。
    *   跳過「是否覆寫」或「移除警告」等互動式確認，適用於自動化部署腳本。
*   **原子化變更 (change)**：支援重新加密現有金庫，並支援 `--commit` 自動提交至 Git。

#### **3.3 Git 自動化同步 (jkim sync)**
*   **原子化備份流程**：自動執行 `git add .` -> `git commit` (帶時間戳記) -> `git pull --rebase` -> `git push`。

### ---

**第四章：數據層與安全性 (Technical Spec)**

#### **4.1 認證體系 (Hardened Auth)**
*   **交互式輸入 (Indicator)**：實現「切換式狀態指示器」 (`[ * ]` / `[ x ]`)。
*   **優先順序 (Precedence)**：
    1.  **強制互動模式** (`-I`)。
    2.  **0600 靜態金鑰檔案** (預設 `~/.config/jki/master.key`)。
    3.  **Agent Session** (記憶體快取)。
    4.  **自動回退互動式詢問** (Stdin)。

#### **4.2 代理服務與 IPC (jki-agent)**
*   **通訊協定**：採用 `interprocess` 實作跨平台 Local Sockets，透過 JSON 進行指令交換。

#### **4.3 環境變數與路徑保護**
*   `JKI_HOME` 等路徑覆寫。
*   **`vault.metadata.json`**：僅含搜尋 Metadata。
*   **`vault.secrets.bin.age`**：整包加密之秘密資料庫。

### ---

**第五章：實作路徑 (Roadmap)**

1.  **Phase 1: Foundation**：WORKSPACE 建立 (Done)。
2.  **Phase 2: Core Executor (jki)**：Rust 實作、資料拆分加密 (Done)。
3.  **Phase 3: Management (jkim)**：Git 同步、編輯器模式、Master Key 工具集 (Done)。
4.  **Phase 4: Agent & Key Caching (jki-agent)**：實作 Agent 快取機制與 Keychain 串接。
5.  **Phase 5: Refinement**：二進位優化 (rkyv)、安裝腳本。
