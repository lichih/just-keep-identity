# Mission: Vault State & Transition Implementation (Phase 4 Extension)

## 1. Objective
實作「金庫狀態感知 (Vault State Awareness)」機制。核心目標是支援「脫殼模式 (Plaintext Mode)」，在磁碟存在 `vault.secrets.json` 時繞過所有認證流程以達成零延遲查詢，並提供安全可靠的狀態轉換工具。

## 2. Technical Contract (核心規範)
### 2.1 搜尋優先序 (Search Priority)
`jki` 執行時應依序嘗試以下路徑，一旦成功即回傳結果：
1.  **Plaintext Path**: 讀取 `vault.secrets.json` (最快，跳過認證)。
2.  **Agent Path**: 透過 IPC 請求 `jki-agent` (需 Socket 連接)。
3.  **Static Key Path**: 使用 `master.key` 解鎖 `.age` (自動解密)。
4.  **Interactive Path**: 提示輸入 Master Key。

### 2.2 搜尋哲學 (Search Philosophy)
*   **Field Isolation**: 單一 Pattern 必須完全落在 `Issuer` 或 `Account Name` 之一，嚴禁跨欄位匹配（防止 `gh` 誤中 `Google-lichih`）。
*   **Multi-pattern AND**: 多個 Pattern（空格分隔）採交集邏輯。

## 3. Implementation Tasks
### 3.1 Core Logic (`jki-core`)
- [x] `paths.rs`: 實作 `decrypted_secrets_path()`。
- [x] `lib.rs`: 升級 `search_accounts` 以支援「欄位隔離」與「多關鍵字 AND」。
- [x] `lib.rs`: 修正 `fuzzy_match` 確保字元消耗順序嚴格。

### 3.2 CLI 升級 (`jki` & `jkim`)
- [x] `jki`: 更新 `run()` 流程，導入上述搜尋優先序。
- [x] `jkim`: 實作 `decrypt` 指令（`.age` -> `.json`，預設保留 `master.key`）。
- [x] `jkim`: 實作 `encrypt` 指令（`.json` -> `.age`，並物理刪除 `.json`）。
- [x] `jkim`: 重構 `import-winauth`。偵測 `Hybrid` 狀態，若具備 `master.key` 則自動執行加密封裝。支援 `-y, --yes` 跳過衝突確認。

## 4. Testing & Validation (測試規範)
### 4.1 單元測試 (Unit Tests)
- [x] **搜尋測試**: 驗證 `gh` 僅匹配 `GitHub` 而不匹配包含 `g` 和 `h` 的 `Google` 帳號。
- [x] **狀態轉換測試**: 驗證 `jkim decrypt` 後 `master.key` 是否保留，且 `jki` 能否正確讀取明文。
- [x] **衝突測試**: 驗證在 `Hybrid` 狀態下，`import` 指令在有/無 `-y` 旗標時的行為。

### 4.2 物理驗證
- [x] 執行 `cargo test` 確保所有測試通過。
- [x] 手動測試極速模式：建立 `vault.secrets.json` 後，`jki` 應在不啟動 Agent 的情況下直接輸出 OTP。

## 5. Completion & Reporting (結案要求)
任務完成後，請提供一份 **結案報告**，包含：
1.  **實作摘要**: 簡述異動的檔案與新增的功能。
2.  **測試報告**: `cargo test` 的執行結果截圖或文字紀錄。
3.  **規格對照**: 確認所有行為均符合 `docs/jki-prd-spec.md` (V24+) 與 `docs/jki-cli-spec.md` 的描述。

---
*Generated for next session handover. Reference files: docs/jki-prd-spec.md, docs/jki-cli-spec.md*
