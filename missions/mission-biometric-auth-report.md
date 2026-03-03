# Mission Report: 生物辨識 (Biometric) 原生整合

本任務已完成 `jki-agent` 與 macOS `LocalAuthentication` 的原生整合，並實現了透過 Keychain 儲存 Master Key 的生物辨識解鎖流程。

## 1. 核心任務達成度 (Task Completion)

- [x] **Dependency 補強**:
    - macOS: 引入了 `objc`, `block` 與 `security-framework` 並新增 `build.rs` 連結系統框架。
- [x] **實作 Biometric Driver (`biometric.rs`)**:
    - 封裝 `verify_biometric(reason: &str)` 函式，透過 `objc` 觸發 macOS TouchID/FaceID。
- [x] **Keychain 授權聯動**:
    - 實作 `unlock_with_biometric` 流程：驗證指紋成功後，從 Keychain (`keyring`) 獲取 Master Key 進行解鎖。
- [x] **Agent 邏輯整合**:
    - 支援 `-A biometric` 啟動分支，並在 `jki-core` 加入 `UnlockBiometric` IPC 指令供 CLI 遠端調用。
- [x] **Tray 選單更新**:
    - 加入 "Unlock with Biometric" 選項，並根據 Agent 鎖定狀態動態啟用/禁用。

## 2. 實作細節 (Implementation Details)

- **IPC 擴展**: `AgentClient` 新增 `unlock_biometric()` 方法，允許 CLI 無須密碼即可要求 Agent 觸發 OS 原生驗證。
- **UI 互動**: Tray 選單中的 "Unlock with Biometric" 與 "Lock" 互斥，確保操作邏輯清晰。
- **安全性**: Master Key 僅在生物辨識驗證成功後才從 Keychain 讀取，並立即用於解鎖 Vault。

## 3. 驗證結果 (Verification)

- **編譯**: `cargo build` 成功。
- **測試**: `cargo test -p jki-agent` 全部通過。
- **環境**: 已在 macOS 環境下確認 `LocalAuthentication` 連結正常。

---
*Status: Completed. High-Privilege Biometric Auth Layer established.*
