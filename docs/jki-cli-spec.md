# **Just Keep Identity (jki)：CLI 指令手冊 (Command-Line Interface Spec)**

這份文件詳細記載 `jki` 與 `jkim` 的所有指令、旗標與預期行為，作為實作與測試的依準。

---

## **1. 全域旗標 (Global Flags)**

適用於所有 `jki` 與 `jkim` 指令：
*   `-A, --auth <SOURCE>`: 指定認證權威來源。支援 `auto`, `agent`, `plain`, `mkey`, `interactive`。
*   `-I, --interactive`: 強制互動模式（等同於 `-A interactive`）。
*   `-q, --quiet`: 安靜模式。對「已預先授權」訊息執行噪聲抑制。
*   `-d, --default`: 自動模式。對於所有具備建議偏好的詢問（如狀態轉換、匯入衝突），自動套用系統推薦行為。

---

## **1.1 授權與抑制矩陣 (Authorization & Suppression Matrix)**

所有變動性操作（如 `add`, `edit`, `sync`, `init`）必須遵循此權威矩陣：

| 授權狀態 (Auth) | 噪聲要求 (Quiet) | 系統行為 (Behavior) |
| :--- | :--- | :--- |
| **未授權 (Default)** | **非安靜 (Default)** | **互動保護**：執行完整安全檢查、導引與握手。 |
| **未授權** | **安靜 (-q)** | **無效抑制**：忽略 `-q` 對核心安全流程的抑制，仍執行互動保護。 |
| **已授權 (-f, -d)** | **非安靜** | **輕量導引**：執行操作，顯示非阻斷性的操作摘要或即時回饋。 |
| **已授權 (-f, -d)** | **安靜 (-q)** | **全速自動化**：靜默執行物理操作，僅在發生「阻斷性錯誤」時輸出 stderr。 |

---

## **1.2 認證優先序 (Authentication Priority)**

當指令需要解鎖加密金庫（`.age`）且未指定 `-A`時，依序嘗試：
1.  **Agent Session**: 透過 IPC 請求 `jki-agent` (最高優先)。
2.  **Plaintext Vault**: 讀取 `vault.secrets.json`。
3.  **Master Key File**: 讀取 `$JKI_HOME/master.key` (0600)。
4.  **Interactive Prompt**: 開啟終端機密碼輸入。

---

## **2. 執行器：jki**

### **2.1 搜尋與 OTP 生成**
`jki [PATTERNS]... [INDEX] [-A <SOURCE>]`

#### **旗標**
*   `-s, --stdout`: 直接在 stdout 印出 OTP。
*   `-`: 等同於 `--stdout`。
*   `-l, --list`: 強制顯示匹配清單。
*   `-o, --otp`: 在清單模式下顯示 OTP。
*   `-S, --show-secret`: 唯一匹配時，顯示原始 Base32 Secret 而非 OTP。
*   `-U, --uri`: 唯一匹配時，顯示完整 OTPAuth URI 而非 OTP。

---

## **3. 管理中心：jkim**

### **3.1 狀態檢查 (status)**
`jkim status`
*   檢查金鑰檔案權限、系統 Keychain 紀錄、Agent 運行狀態及 Git 同步狀況。

### **3.2 代理服務管理 (agent)**
`jkim agent <SUBCOMMAND>`
*   `start`: 啟動背景 `jki-agent`。採用 Detach 模式，啟動後脫離終端進程組。
*   `stop`: 優雅關閉正在運行的 `jki-agent`。
*   `restart`: 重啟代理服務。

### 3.3 環境初始化 (init)
`jkim init [--force]`
*   初始化 JKI 工作目錄與 Git 儲存庫。使用 `-f` 可執行物理重置。

### 3.4 Git 同步 (git)
`jkim git`
*   執行標準同步流程：`git add .` -> `git commit` -> `git pull --rebase` -> `git push`。
*   支援別名 `sync` 以維持相容性。
`jkim add [NAME] [ISSUER] [--secret <SECRET>] [--uri <URI>] [-f/--force] [-S/--show-secret] [--stdout]`
*   手動新增 OTP 帳號。
*   **物理握手 (Live Handshake)**：
    *   在 TTY 模式下，存檔前會進入動態產碼循環，顯示即時 OTP 供使用者向服務商驗證。
    *   **剪貼簿自動化**：握手期間，系統會自動將最新的 OTP 碼複製到剪貼簿（除非指定了 `--stdout` 或 `-`），畫面上會顯示 `(Copied!)`。
    *   **按下 ENTER**：確認驗證通過，執行物理寫入。
    - **按下 CTRL-C**：取消操作，不更動金庫。
*   **參數**:
    *   `NAME`: 帳號名稱（如 Email）。
    *   `ISSUER`: 發行者名稱（如 Google）。
    *   `-s, --secret`: 直接提供 Base32 金鑰。會自動執行 `trim()`、空格移除與轉大寫。
    *   `--uri`: 從 `otpauth://` URI 匯入。
    *   `-f, --force`: 若名稱與發行者重複，強制覆蓋現有分錄。
    *   `-S, --show-secret`: 成功新增後，印出原始 Base32 Secret 與 OTPAuth URI。
    *   `--stdout`: 僅在 stdout 輸出，握手期間不觸碰剪貼簿。
    *   `-`: 等同於 `--stdout`。
*   **授權矩陣應用**：若同時指定 `-f` (或 `-d`) 與 `-q`，則跳過握手執行靜默寫入。

*   **安全特性**: 在 TTY 模式下若直接提供 `--secret`，會發出 History 洩漏警告。

### 3.5 金鑰管理 (master-key)

`jkim master-key <SUBCOMMAND>`
*   `set [--force] [--keychain]`: 將金鑰寫入磁碟，預設同步寫入 Keychain。
*   `remove [--force] [--keychain]`: 從磁碟與 Keychain 移除金鑰。
*   `change [--commit]`: 執行金鑰輪轉，重新加密金庫並更新系統紀錄。

### **3.6 系統金鑰鏈工具 (keychain)**
`jkim keychain <SUBCOMMAND>`
*   `set`: 在終端機安全輸入 Master Key 並直接寫入系統 Keychain（具備 ACL 授權）。
*   `push`: 將本地 `master.key` 內容寫入系統 Keychain。
*   `pull`: 將系統 Keychain 中的金鑰讀取並存回本地 `master.key`。
*   `remove`: 徹底刪除系統 Keychain 中的 `jki:master_key`項目。

### **3.7 資料管理 (Vault Management)**
*   **decrypt**: 將金庫轉換為明文 JSON。
*   **encrypt**: 將明文金庫壓回加密的 `.age` 檔案。
*   **import-winauth <FILE>**: 從 WinAuth 匯出檔批次匯入帳號。
*   **export [--output <FILE>]**: 匯出加密的 ZIP 備份包（包含 OTPAuth URI 清單）。

### **3.8 資料編輯 (edit)**
`jkim edit`
*   呼叫 `$EDITOR` 編輯 Metadata。存檔後自動執行格式驗證並通知 Agent 重載。

### **3.9 去重工具 (dedupe)**
`jkim dedupe [-k <IDX>] [-d <IDX>] [-y/--yes]`
*   **指紋掃描 (Fingerprint Scanning)**：自動解密金庫並根據 Decrypted Secrets 進行分組，為每個帳號分配全域唯一序號。
*   **Mark-and-Sweep 策略**：
    *   **無參數**：列出所有重複分組與序號。
    *   **`-k, --keep <IDX>` (Keep)**：組內排除法。標記保留此項，並將該組內其餘所有項目標記為刪除。
    *   **`-d, --discard <IDX>` (Discard)**：標記刪除指定序號。
*   **安全流程**：
    *   物理刪除前會顯示明細並要求二次確認 (除非指定了 `-y` 或 `--yes`)。
    *   刪除後自動通知 Agent 重載。

---

## **4. 輸出規範 (Output Standards)**

### **4.1 訊息流向與狀態碼**
*   **stderr**: 用於提示、警告、互動詢問、密碼輸入與**狀態引導 (Tips)**。
*   **stdout**: 僅用於純淨的資料輸出（如 OTP、JSON）。
*   **搜尋結果狀態**: 
    *   **結果唯一 (Single Match)**: 執行動作 (Execute/List) 並以 Exit Code 0 退出。
    *   **結果多於一項 (Multiple Matches)**: 列出清單 (List) 並以 Exit Code 0 退出。不視為錯誤。
    *   **查無結果 (No Match)**: 提示訊息並以 Exit Code 1 報錯退出。

### **4.2 輸出標籤 (Labels)**
*   **`Accounts:`**: 使用 `jki` 或 `jki -l` 且未提供搜尋 Pattern 時的標題。
*   **`Matches:`**: 有提供搜尋 Pattern 且搜尋結果不唯一，或強制使用 `-l` 時的標題。
*   **`Ambiguous results:`**: 僅在有搜尋 Pattern 且結果不唯一，且**未**指定 `-l` 時，作為導引性標題。

### **4.3 衝突處理規範 (Conflict Handling)**
*   當發生「狀態衝突」（如同步衝突）時，強制使用者確認。支援 `-d, --default` 或 `-y, --yes` 套用系統推薦行為。

---

## **附錄 B：代理服務啟動政策 (Agent Lifecycle Policy)**

為維護系統紀律與最小驚訝原則，`jki` 遵循以下政策：
1.  **禁止自動啟動**：`jki` 查詢指令絕對禁止在 Agent 未運行時主動啟動進程。
2.  **被動解鎖 (Passive Unlock)**：若 `jki-agent` 已在運行但處於 Locked 狀態，`jki` 獲得 Master Key 後應嘗試請求 Agent 解鎖 (Lazy Unlock)。
3.  **顯式啟動**：使用者必須透過 `jkim agent start` 或 OS 層級的啟動項顯式開啟服務。
4.  **引導提示**：若 `jki` 執行時未偵測到 Agent，應在 stderr 顯示輕量級建議 (Tip)。

---

## **附錄 C：參數解析與序號決策 (Pattern & Index Resolution)**

為平衡操作速度與意圖精確性，`jki` 的參數解析遵循以下智慧決策流程：

### **C.1 核心邏輯層次**
1.  **過濾層 (Filter Chain)**:
    *   **Pattern Filter**: 執行基於 `fuzzy-matcher` (Skim 演算法) 的模糊搜尋，支援權重計分與匹配字元高亮。
    *   **Index Filter**: 若最後一個參數為純數字（且未受 `--` 保護），則從 Pattern 結果中選取對應項。
    *   **Final Results**: 過濾鏈最終產出的結果集。
2.  **動作層 (Action Selection)**:
    *   **Execute**: 當結果唯一且未指定 `-l` 時，執行 OTP 生成。
    *   **List**: 當結果不唯一，或明確指定 `-l` 時，僅印出 `Final Results` 清單。

### **C.2 決策矩陣 (Decision Matrix)**

| 輸入模式 | 過濾結果 | 是否帶 `-l` | 最終行為 | 輸出標題 |
| :--- | :--- | :--- | :--- | :--- |
| `jki` (無參數) | 全部帳號 | (不論) | 列出所有帳號 | `Accounts:` |
| `jki <P>` | > 1 個結果 | 否 | 列出清單 + Tip | `Ambiguous results:` |
| `jki <P>` | > 1 個結果 | 是 | 列出符合清單 | `Matches:` |
| `jki <P> <IDX>` | 有效 (結果=1) | 否 | **執行** (Execute) | 無 |
| `jki <P> <IDX>` | 有效 (結果=1) | 是 | 列出該單項 | `Matches:` |
| `jki <P> <IDX>` | 無效 (IDX 超出) | (任何) | 降級為 P 與 IDX 合併搜尋 | `Matches:` |
| `jki -- <ARGS>` | (不論) | (不論) | 強制將所有 ARGS 視為搜尋模式 | (依結果數) |
