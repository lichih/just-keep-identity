# Mission: jki-agent Security Hardening (--force-age)

## 1. Objective
強化 `jki-agent` 的安全性，新增 `--force-age` 旗標與環境變數支援，確保 Agent 僅能從加密來源 (`.age`) 加載金庫。根據要求，**此任務不應修改 `jki` 客戶端程式碼**。

## 2. Tasks
- [x] **Agent CLI Argument (`crates/jki-agent/src/main.rs`)**:
    - 使用 `clap` 為 `jki-agent` 新增 `force_age: bool` 旗標。
- [x] **Environment Variable Support**:
    - 在 `main` 函式中，若偵測到環境變數 `JKI_FORCE_AGE=1`，則自動啟動 `force_age` 模式。
- [x] **Unlock Logic Hardening**:
    - 修改 `State::unlock`：若 `force_age` 模式開啟，跳過對 `vault.secrets.json` 的檢查。
    - 若此時 `.age` 檔案缺失，回傳精確錯誤：「Force-age mode enabled: Encrypted vault missing. Refusing to load plaintext.」
- [x] **Verification**:
    - 模擬環境：僅存在 `vault.secrets.json`。
    - 測試 1：手動啟動 `jki-agent --force-age` 並嘗試解鎖。預期：解鎖失敗。
    - 測試 2：設定 `export JKI_FORCE_AGE=1` 後啟動 `jki-agent`。預期：解鎖失敗。

## 3. Deliverables
- [x] 修改後的 `jki-agent` 原始碼。
- [x] 驗證報告 `missions/mission-jki-agent-hardening-report.md`。
