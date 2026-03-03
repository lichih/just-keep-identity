# Mission Report: jki Fail-Fast Logic & Precise Diagnostics

## 1. Summary
本任務成功落實了 `jki` 客戶端與 `jki-agent` 的 Fail-Fast 原則，提升了錯誤診斷的精確度，並移除了潛在的 Panic。

## 2. Changes
### jki-core
- 修改 `agent::Response::Unlocked` 為 `Unlocked(String)`，允許攜帶金庫加載來源資訊（如 "Encrypted Vault" 或 "Plaintext Vault"）。

### jki-agent
- **State::unlock 增強**：現在支援在 `.age` 檔案缺失時，自動尋找 `.json` (明文金庫) 作為備選加載來源。
- **錯誤回報**：若兩者皆缺失，回傳精確的錯誤訊息而非 generic error。

### jki (Client)
- **Strict Agent Mode**：若使用 `--force-agent`，當 Agent 無法提供 OTP 時（例如 Agent 未啟動、連線失敗或解密失敗），程式將直接報錯並結束，不再回退到本地路徑。
- **Error Propagation**：
    - 在 `jki agent unlock` 命令中，現在會顯示 Agent 回報的金庫來源。
    - 在獲取 OTP 流程中，若 Agent 回報錯誤（例如金庫檔案遺失），客戶端會擷取並顯示該錯誤訊息。
- **Panic Removal**：
    - 將本地路徑中的 `.expect("Secrets file missing")` 替換為友善的錯誤提示：「金庫檔案遺失，請執行 jkim init 或恢復備份。」
- **Robustness**：
    - 改進了 Agent 執行檔的自動尋找邏輯，使其能正確處理 `cargo test` 環境（`deps` 資料夾）。

## 3. Verification Results
### 測試 1：無金庫檔案執行 `jki`
- **操作**：刪除 `.age` 與 `.json` 後執行 `jki google`。
- **結果**：顯示「Error: Secrets file missing at ... Please run jkim init or restore from backup.」並正常退出（Exit Code 1），無 Panic。

### 測試 2：Agent 備選路徑驗證
- **操作**：刪除 `.age` 但保留 `.json`，執行 `jki --force-agent`。
- **結果**：Agent 成功加載明文金庫，`jki` 成功取得 OTP。

### 測試 3：`--force-agent` Fail-Fast 驗證
- **操作**：在金庫遺失情況下執行 `jki --force-agent`。
- **結果**：顯示 Agent 的詳細錯誤訊息，並提示「Error: Agent path failed and --force-agent is enabled. Bailing out.」，未嘗試進入本地路徑。

### 測試 4：單元測試
- 所有相關單元測試（`jki`, `jki-agent`, `jki-core`, `jkim`）均全數通過。

## 4. Conclusion
系統現在具備更強健的錯誤處理能力，使用者能透過錯誤訊息明確得知金庫狀態與解密路徑。
