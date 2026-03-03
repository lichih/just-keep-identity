# Mission Report: Keychain Authentication Integration

## 1. Summary
本任務成功將 `KeyringStore` (利用系統原生 Keychain/Keyring) 整合至 `jki` 核心認證流程。實現了「Keychain 優先」的 Master Key 安全存取策略，優化了使用者體驗，減少了手動輸入密碼的頻率，並強化了金鑰管理的安全性。

## 2. Changes

### crates/jki-core/src/keychain.rs
- 實作 `SecretStore` trait 與 `KeyringStore` 結構。
- `KeyringStore` 利用 `keyring` crate 對接 macOS Keychain 與 Windows Credential Manager。
- 新增 `MockSecretStore` 用於單元測試。

### crates/jki-core/src/lib.rs
- 重構 `acquire_master_key` 函式：
    - 新增 `secret_store: Option<&dyn SecretStore>` 參數。
    - 調整認證優先序：
        1. `force_interactive` (旗標 `-I`)。
        2. **System Keychain** (從 `jki:master_key` 讀取)。
        3. **Master Key File** (`master.key` 檔案)。
        4. **Interactive Prompt** (手動輸入)。
- 新增 `test_acquire_master_key_priority` 測試案例驗證優先序邏輯。

### crates/jki/src/main.rs
- 在 `run` 函式中實例化 `KeyringStore` 並傳遞給 `acquire_master_key`。
- 確保 `jki` 優先嘗試從 Keychain 獲取金鑰以解鎖金庫。

### crates/jkim/src/main.rs
- 更新 `handle_status`: 新增系統 Keychain 狀態檢查，顯示是否已存有 `jki:master_key`。
- 更新 `handle_master_key`:
    - `set` 指令：新增 `--keychain` 旗標 (預設為 true)，成功設定後將金鑰同步存入系統 Keychain。
    - `remove` 指令：新增 `--keychain` 旗標，同步移除系統 Keychain 中的金鑰。
    - `change` 指令：金鑰變更後，自動更新系統 Keychain 中的內容。
- 更新 `handle_import_winauth`, `handle_decrypt`, `handle_encrypt`, `handle_export`: 全面採用 `KeyringStore` 進行認證。

### crates/jki-agent/src/main.rs (Bug Fix)
- 為測試案例添加 `#[serial]` 標記，解決因環境變數競爭導致的測試失敗問題。

## 3. Verification Results
- **單元測試**: 執行 `cargo test` 全數通過 (共 39 個測試案例)。
    - `jki-core`: 20 passed.
    - `jki`: 5 passed.
    - `jkim`: 9 passed.
    - `jki-agent`: 5 passed (已修正 `test_handle_client_unlock_and_get_otp` 失敗問題)。
- **手動驗證**:
    - [x] `jkim master-key set --keychain`: 成功寫入 `master.key` 並同步至系統 Keychain。
    - [x] `jkim status`: 正確顯示 `System Keychain: Found (jki:master_key)`。
    - [x] `jki`: 在有 Keychain 記錄時，直接解鎖金庫並生成 OTP，無須輸入密碼。
    - [x] `jki -I`: 成功忽略 Keychain，強制開啟互動式密碼輸入。

## 4. Conclusion
Keychain 整合現已完整實作並通過驗證。系統現在能更安全且便捷地管理 Master Key，符合 PRD 中對於安全性與易用性的平衡要求。
