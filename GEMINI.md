# Just Keep Identity (JKI) - Project Mandates

本文件記載本專案的基礎架構決策與工程準則。所有開發任務與子代理 (Sub-Agent) 必須嚴格遵守。

## 1. 認證架構與自動化原則 (Authentication & Automation)

為確保單元測試與 CI/CD 流程不被作業系統 (OS) 授權視窗（如 macOS Keychain ACL）中斷，`jki` 的 Master Key 獲取優先序必須固定為：

1.  **Master Key File (`master.key`)**: 優先讀取物理檔案。
2.  **Agent Session**: 向背景代理請求已解鎖的金鑰。
3.  **System Keyring**: 最後才嘗試存取系統金鑰鏈（僅適用於已授權的正式環境）。

**任何涉及金鑰操作的單元測試，必須確保在存在物理檔案的情況下能「靜默」通過。**

## 2. CLI 防禦性設計準則 (Defensive CLI Design)

### 授權與行為邊界 (SSoT)：
*   **核心指令行為 (Authorization & Quiet Mode)**：
    任何涉及旗標行為（尤其是 `-f`, `-d`, `-q`）的開發與變更，**必須強制讀取並嚴格遵循 `docs/jki-cli-spec.md` 章節 1.1 中的「授權與抑制矩陣」**。這是系統物理誠信的唯一權威來源。

### 安靜模式 (`-q`) 的行為底線：
*   **阻斷性錯誤 (Conflict/IO)**: 即使在 `-q` 模式下，失敗時**必須**於 `stderr` 噴出明確錯誤訊息。
*   **成功路徑**: 在 `-q` 模式下，成功達成任務後應保持完全靜默。

### 強制模式 (`-f`) 的定義：
*   `add -f` 代表「強制新增」，即產生新 UUID 寫入。**絕不執行自動覆蓋行為**，以保護物理資料的完整性。

## 3. 物理誠信原則 (Physical Integrity)
*   所有金鑰輸入（如 `add` 的 Secret）在 TTY 模式下必須使用隱蔽遮罩輸入。
*   金鑰在存入物理檔案前，必須經過 `trim()`, `replace(" ", "")`, `to_uppercase()` 的標準化清理。
