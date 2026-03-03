# **Just Keep Identity (jki)：JK Suite 極速 MFA 數位金庫**

## **產品需求 (PRD) 與技術規格 (Spec) 文件 - V29 (ACL 授權優化與原生效能版)**

### **第一章：品牌與核心原則 (Principles)**

#### **1.4 金庫狀態與認證邏輯 (Vault States & Auth Logic)**
*   **無感解鎖 (Lazy Unlock)**：`jki-agent` 啟動時預設為 Locked。當 `jki` 執行查詢時，若發現 Agent 已鎖定，應根據啟動配置主動調用認證（如 Biometric）並實現 Session 快取。
*   **認證職責隔離 (Auth Separation)**：
    *   **Agent (High-Privilege)**：唯一的系統級安全框架對接口。負責調用 **Biometric (OS 原生生物辨識)** 或系統 Keychain。
    *   **CLI (Lightweight)**：獨立運作時不調用系統級安全框架，僅限檔案或直接互動。

### ---

**第二章：架構定義 (Architecture)**

#### **2.2 代理服務與可視化管理 (jki-agent)**
*   **定位**：唯一的 Session 管理器與高權限認證門戶。負責在記憶體中快取解密後的 Secrets 與 Master Key。
*   **自治解鎖**：若 Agent 以 `-A biometric` 啟動，當收到查詢請求且處於 Locked 狀態時，應自動發起系統驗證，對 CLI 透明。
*   **跨平台形態**：macOS/Windows 提供選單列圖示 (Menu Bar)，Linux CLI 為純背景 Daemon。

#### **2.3 權威來源旗標 (-A, --auth)**
為顯式指定認證來源並實現 Fail-fast 策略，所有組件支援 `-A, --auth <SOURCE>` 參數。

| 參數值 (`-A`) | 行為 (Behavior) | 適用組件 |
| :--- | :--- | :--- |
| **`biometric`** | 強制調用 **OS 原生生物辨識** (macOS TouchID / Windows Hello)。 | `agent` |
| **`agent`** | 強制僅向 `jki-agent` 請求 (Session 快取)。 | `jki`, `jkim` |
| **`plain`** | 強制僅讀取 `vault.json` (零延遲明文)。 | 全組件 |
| **`mkey`** | 強制僅讀取物理 `master.key` 檔案 (0600)。 | 全組件 |
| **`interactive`** | 強制 Stdin 互動輸入 (Ask Pwd)。別名 `-I`。 | 全組件 |

**認證優先序路徑 (Default Priority Path):**
*   **CLI Path**: `Agent` > `Plain` > `MasterKey` > `Interactive`.
*   **Agent Path**: `Biometric` > `MasterKey` > `Interactive`.

### ---

**第三章：安全硬化標準 (Security Hardening)**

#### **3.1 代理通訊安全**
*   Local Socket 必須強制執行 **0600 (Owner Only)** 權限。
*   Master Key 在 Agent 記憶體中應使用 `SecretString` 保護。

#### **3.2 Keychain ACL 權限管理 (macOS 特色)**
*   **信任共用**：在寫入系統 Keychain 時，應透過 `security` 指令的 `-T` 參數，同時將 `jkim` 與 `jki-agent` 加入受信任程式清單。
*   **單一彈窗**：正確的 ACL 管理確保了在跨程式（從 `jkim` 到 `jki-agent`）存取同一密鑰時，作業系統僅會彈出一次驗證視窗，消除冗餘授權。

---
*Status: Architecture Baselined (V29 - ACL Optimized).*
