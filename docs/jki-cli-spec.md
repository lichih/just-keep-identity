# **Just Keep Identity (jki)：CLI 指令手冊 (Command-Line Interface Spec)**

這份文件詳細記載 `jki` 與 `jkim` 的所有指令、旗標與預期行為，作為實作與測試的依準。

---

## **1. 全域旗標 (Global Flags)**

適用於所有 `jki` 與 `jkim` 指令：
*   `-I, --interactive`: 強制互動模式。忽略磁碟上的 `master.key` 與 Agent 快取，直接開啟密碼輸入。
*   `-q, --quiet`: 安靜模式。抑制 stderr 的提示、進度訊息與非關鍵警告。

---

## **2. 執行器：jki**

### **2.1 搜尋與 OTP 生成 (預設行為)**
`jki [PATTERNS]... [INDEX]`
*   **PATTERNS**: 關鍵字篩選（不分大小寫，模糊匹配）。
*   **INDEX**: 當多個結果時，可指定數字選取。
*   **旗標**:
    *   `-s, --stdout`: 直接在 stdout 印出 OTP（預設為複製到剪貼簿）。
    *   `-`: 等同於 `--stdout`。
    *   `-l, --list`: 強制顯示匹配清單，即使只有一個結果。
    *   `-o, --otp`: 在清單模式下顯示 OTP。

### **2.2 Agent 互動**
`jki agent <SUBCOMMAND>`
*   `ping`: 檢查 Agent 是否存活。
*   `get <ID>`: 透過 Agent 獲取指定 ID 的 OTP（不經由本地解密）。

---

## **3. 管理中心：jkim**

### **3.1 環境初始化 (init)**
`jkim init [--force]`
*   **預設行為 (Transparent Init)**：
    *   檢查並回報目錄狀態。
    *   建立 `.gitignore` 與 `.gitattributes`。
    *   **衝突偵測**：若目錄內已存有 `vault.*` 檔案，印出 **[Data Warning]**。
*   **旗標**:
    *   `-f, --force`: **重置模式**。刪除現有的 `vault.metadata.json` 與 `vault.secrets.bin.age`，重新建立乾淨的金庫環境。

### **3.2 金鑰管理 (master-key)**
`jkim master-key <SUBCOMMAND>`
*   `set [--force]`: 將 Master Key 寫入 `master.key` (0600)。
*   `remove [--force]`: 刪除磁碟上的 `master.key`。
*   `change [--commit]`: 
    1. 讀取舊金鑰（支援 `-I` 手動輸入）。
    2. 解密現有金庫。
    3. 設定新金鑰。
    4. 原子化重新加密並覆寫。

### **3.3 資料編輯 (edit)**
`jkim edit`
*   採用 `crontab -e` 流程。
*   開啟 `$EDITOR` 編輯 `metadata.json` 的臨時副本。
*   儲存後自動執行 JSON Schema 驗證，成功後才覆寫正式檔案。

### **3.4 同步 (sync)**
`jkim sync`
*   執行 Git 原子化備份：`add` -> `commit` -> `pull --rebase` -> `push`。

---

## **4. 輸出規範 (Output Standards)**

### **4.1 訊息流向**
*   **stderr**: 用於提示、警告、進度報告、以及「切換式狀態指示器」密碼輸入。
*   **stdout**: 僅用於純淨的資料輸出（如 OTP 碼、JSON 導出）。

### **4.2 錯誤處理**
*   **Fail-Fast**: 密碼錯誤或環境不安全時，應立即印出錯誤訊息並退出，不進行不必要的重試。
*   **Exit Codes**:
    *   `0`: 成功。
    *   `1`: 一般錯誤 (如查無帳號)。
    *   `100-110`: 環境/安全性錯誤。
