# **Just Keep Identity (jki)：JK Suite 極速 MFA 數位金庫**

## **產品需求 (PRD) 與技術規格 (Spec) 文件 - V19 (最終交互與規範定案版)**

### **第一章：品牌與體系定義 (Brand & System)**

#### **1.1 品牌命名決策**
*   **正式名稱**：**Just Keep Identity (jki)**。
*   **產品線主題**：**「Just Keep 系列」**，核心精神為「身分主權的極速掌控」。
*   **人體工學**：主打 **右手單手操作 (Micro-Roll)**，指令配置（j-k-i）確保手指位移最小化。

#### **1.2 指令體系架構 (Command Hierarchy)**
*   **jki (Hot Path)**：極速執行器。預設執行「交集搜尋」與「OTP 複製」。追求 < 3ms 啟動，無動態補完以保證效能。
*   **jkim (Management)**：管理中心。負責 CRUD、WinAuth 匯入、Git 加密同步、rkyv 二進位快取優化。提供 TUI 與動態補完。
*   **jki-agent (Service)**：背景授權服務。負責維持 Session (TTL)、主金鑰記憶體快取與跨平台 IPC。
*   **jki-core (Core)**：共享邏輯庫。包含加密、TOTP 演算法與數據定義，供上述三者調用。

### ---

**第二章：交互邏輯與 Unix 工具鏈規範 (CLI Standards)**

#### **2.1 查詢與選取行為**
*   **單一結果**：自動複製 OTP 至剪貼簿，發送系統通知（除非指定 `-`）。
*   **複數結果**：於 stdout 列出結果並標註 **Index (行號)**。
*   **精確選取**：支援語法 `jki [Patterns] [Index]`（例如 `jki gm 2` 直接複製該結果）。

#### **2.2 Unix 風格規範**
*   **Exit Code**：
    *   `0`: 成功 (Success/Copied)。
    *   `1`: 查無匹配 (No Match)。
    *   `2`: 歧義/複數結果 (Ambiguous/List displayed)。
*   **特殊參數**：
    *   `-` (Dash)：將 OTP 直接輸出至 stdout 而非剪貼簿（便於管道整合）。
    *   `--` (Double Dash)：終止選項解析，後續字串強轉為 Pattern，解決與 `-` 或 `--list` 的語義衝突。
*   **標準流 (Streams)**：
    *   **stdout**：輸出 OTP 內容或條目列表。
    *   **stderr**：輸出狀態提示、錯誤訊息與系統通知文字。

### ---

**第三章：數據、安全與性能 (Technical Spec)**

#### **3.1 數據層與安全**
*   **存儲**：底層使用 JSON，透過 Git age 過濾器 (`.gitattributes`) 進行加密同步。
*   **優化**：`jkim optimize` 產生 **rkyv 二進位快取** (Zero-copy)，`jki` 優先讀取以達成極速啟動。
*   **硬體安全性**：
    *   **Windows Hello ESS**：原生呼叫系統生體辨識。
    *   **WSL2 橋接**：透過 Windows 端代理確保 Linux 環境仍可喚起生體辨識。

#### **3.2 補完功能邊界**
*   **jki**：保持 **Completion-free**，避免載入補完腳本造成的延遲。
*   **jkim**：基於 Clap 生成動態補完，並在 TUI 中提供智慧型帳號建議 (Email Suggestion)。

### ---

**第四章：實作路徑 (Roadmap)**

1.  **Phase 1: Foundation (Workspace)**：建立 Rust Workspace，定義 `jki-core` 基礎加密與序列化邏輯。
2.  **Phase 2: Core Executor (jki)**：實作交集搜尋、Index 選取、Exit Code 體系與 `--` 支援。
3.  **Phase 3: Agent & IPC (jki-agent)**：實作背景 Session 管理與 Unix Socket/Named Pipe 通訊，優化「二次查詢」速度。
4.  **Phase 4: Management & TUI (jkim)**：開發 Ratatui 界面、`optimize` 指令與動態補完生成。
5.  **Phase 5: Ecosystem**：安裝腳本、WSL 橋接配置、Shell 補完腳本集成。

#### **技術指標**
*   **jki 啟動延遲**：< 3ms (Cold start)。
*   **IPC 延遲**：< 1ms。
*   **Agent 內存佔用**：< 10MB。

### ---

**第五章：安全性硬化 (Security Hardening)**

#### **5.1 IPC 准入控制 (Access Control)**
*   **身份校驗**：Agent 必須校驗連線進程的 **UID (User ID)**。拒絕非目前使用者的任何請求。
*   **路徑白名單**：(進階) Agent 應檢查呼叫者的 PID 路徑，確保其為受信任的 `jki` 或 `jkim` 二進位檔。

#### **5.2 記憶體防護 (In-Memory Security)**
*   **防交換 (Anti-Swap)**：使用 `mlock` 或相關 Rust crate (`memlock`) 鎖定敏感記憶體分頁。
*   **自動抹除 (Zeroize)**：敏感數據（如 Master Key）在記憶體中必須實作 `Drop` trait 以執行零值化抹除。

#### **5.3 數據最小化輸出**
*   **無密鑰傳輸**：Agent 嚴禁透過 IPC 輸出 Master Key 或 Data Key。
*   **按需計算**：IPC 協議僅支援「傳入 Pattern -> 傳回 OTP 及其剩餘秒數」的閉環操作。
