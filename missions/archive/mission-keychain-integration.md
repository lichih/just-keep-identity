# Mission: Keychain Authentication Integration

## 1. Objective
將 `KeyringStore` (macOS/Windows) 整合至 `jki` 核心認證流程，實現「Keychain 優先」的 Master Key 安全存取。

## 2. Context
- **Files**:
    - `crates/jki-core/src/lib.rs`: `acquire_master_key` 定義。
    - `crates/jki-core/src/keychain.rs`: `SecretStore` 與 `KeyringStore` 實作。
    - `crates/jki/src/main.rs`: `jki` 指令進入點。
    - `crates/jkim/src/main.rs`: `jkim` 管理指令。
- **Status**: `KeyringStore` 已通過 Prototype 驗證。

## 3. Checklist
- [ ] **Core Refactor**: 
    - 修改 `acquire_master_key`，新增 `secret_store: Option<&dyn SecretStore>` 參數。
    - 優先序調整：1. `force_interactive` -> 2. `secret_store` -> 3. `master.key` file -> 4. `prompt_password`。
- [ ] **CLI Update (jki)**: 
    - 在 `jki/src/main.rs` 中實例化 `KeyringStore` 並傳遞給 `acquire_master_key`。
- [ ] **CLI Update (jkim)**:
    - 實例化 `KeyringStore` 並應用於 `handle_master_key` 與其他涉及認證的指令。
    - 修改 `jkim master-key set`: 新增 `--keychain` 旗標（預設為 true），成功後將金鑰存入 Keychain。
    - 修改 `jkim master-key remove`: 新增 `--keychain` 旗標，移除 Keychain 中的記錄。
    - 更新 `handle_status`: 顯示 Keychain 是否已存有金鑰。
- [ ] **Documentation**:
    - 更新 `docs/jki-cli-spec.md` 描述 Keychain 優先權。
- [ ] **Verification**:
    - 執行 `cargo test -p jki-core` 確保原有測試通過。
    - 增加新測試案例至 `jki-core` 驗證 `acquire_master_key` 的優先序。

## 4. Judge Mechanism
- **Pass**: 執行 `cargo test` 全數通過。
- **Pass**: `jkim status` 能正確顯示 Keychain 狀態（即使是 Mock 情境）。
- **Pass**: `jki` 在有 Keychain 記錄時不再提示輸入密碼或尋找 `master.key`。

---
*Created by Main Agent (Orchestrator). Please submit a Closure Report upon completion.*
