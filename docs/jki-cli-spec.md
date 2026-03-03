# **Just Keep Identity (jki)：CLI 指令手冊 (Command-Line Interface Spec)**

這份文件詳細記載 `jki` 與 `jkim` 的所有指令、旗標與預期行為，作為實作與測試的依準。

---

## **1. 全域旗標 (Global Flags)**

適用於所有 `jki` 與 `jkim` 指令：
*   `-A, --auth <SOURCE>`: 指定認證權威來源。支援 `auto`, `agent`, `plain`, `mkey`, `interactive`。
*   `-I, --interactive`: 強制互動模式（等同於 `-A interactive`）。
*   `-q, --quiet`: 安靜模式。抑制 stderr 的提示、進度訊息與非關鍵警告。
*   `-d, --default`: 自動模式。對於所有具備建議偏好的詢問（如狀態轉換、匯入衝突），自動套用系統推薦行為。

---

## **1.1 認證優先序 (Authentication Priority)**

當指令需要解鎖加密金庫（`.age`）且未指定 `-A` 時，依序嘗試：
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

---

## **3. 管理中心：jkim**

### **3.1 狀態檢查 (status)**
`jkim status`
*   檢查金鑰檔案權限、系統 Keychain 紀錄、Agent 運行狀態及 Git 同步狀況。

### **3.2 環境初始化 (init)**
`jkim init [--force]`
*   初始化 JKI 工作目錄與 Git 儲存庫。使用 `-f` 可執行物理重置。

### **3.3 金鑰管理 (master-key)**
`jkim master-key <SUBCOMMAND>`
*   `set [--force] [--keychain]`: 將金鑰寫入磁碟，預設同步寫入 Keychain。
*   `remove [--force] [--keychain]`: 從磁碟與 Keychain 移除金鑰。
*   `change [--commit]`: 執行金鑰輪轉，重新加密金庫並更新系統紀錄。

### **3.4 系統金鑰鏈工具 (keychain)**
`jkim keychain <SUBCOMMAND>`
*   `set`: 在終端機安全輸入 Master Key 並直接寫入系統 Keychain（具備 ACL 授權）。
*   `push`: 將本地 `master.key` 內容寫入系統 Keychain。
*   `pull`: 將系統 Keychain 中的金鑰讀取並存回本地 `master.key`。
*   `remove`: 徹底刪除系統 Keychain 中的 `jki:master_key` 項目。

### **3.5 資料管理 (Vault Management)**
*   **decrypt**: 將金庫轉換為明文 JSON。
*   **encrypt**: 將明文金庫壓回加密的 `.age` 檔案。
*   **import-winauth <FILE>**: 從 WinAuth 匯出檔批次匯入帳號。
*   **export [--output <FILE>]**: 匯出加密的 ZIP 備份包（包含 OTPAuth URI 清單）。

### **3.6 資料編輯 (edit)**
`jkim edit`
*   呼叫 `$EDITOR` 編輯 Metadata。存檔後自動執行格式驗證並通知 Agent 重載。

---

## **4. 輸出規範 (Output Standards)**

### **4.1 訊息流向**
*   **stderr**: 用於提示、警告、互動詢問與密碼輸入。
*   **stdout**: 僅用於純淨的資料輸出（如 OTP、JSON）。

### **4.2 衝突處理規範 (Conflict Handling)**
*   當發生「狀態衝突」（如同步衝突）時，強制使用者確認。支援 `-d, --default` 或 `-y, --yes` 套用系統推薦的安全路徑。
