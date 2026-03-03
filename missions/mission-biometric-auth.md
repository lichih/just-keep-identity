# Mission: 生物辨識 (Biometric) 原生整合

## 1. 背景 (Context)
根據 V28 規格，`jki-agent` 應支援透過 `-A biometric` 旗標啟動 OS 原生生物辨識 (如 macOS TouchID)。這需要整合系統級 API，並與 Keychain 存儲聯動，實現「指紋即解鎖」的極速體驗。

## 2. 核心任務 (Tasks)
- [ ] **Dependency 補強**:
    - [ ] macOS: 引入 `security-framework` 與相關的 Objective-C 綁定。
- [ ] **實作 Biometric Driver (`biometric.rs`)**:
    - [ ] 封裝 `verify_biometric(reason: &str)` 函式。
- [ ] **Keychain 授權聯動**:
    - [ ] 實作受生物辨識保護的 Master Key 讀取邏輯。
- [ ] **Agent 邏輯整合**:
    - [ ] 實作 `-A biometric` 啟動分支。
- [ ] **Tray 選單更新**:
    - [ ] 加入 "Unlock with Biometric" 選項。
- [ ] **結案報告 (Mandatory)**:
    - [ ] **必須執行 `write_file` 產出 `missions/mission-biometric-auth-report.md`。**

## 3. 涉及檔案 (Files Involved)
- `crates/jki-agent/Cargo.toml`
- `crates/jki-agent/src/main.rs`
- `crates/jki-agent/src/tray.rs`
- `crates/jki-agent/src/biometric.rs` (New)
- `missions/mission-biometric-auth-report.md` (New Report)

---
*Status: Delegated by Architect. High-Privilege Auth Focus.*
