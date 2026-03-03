# Mission: jki --force-agent & UX Improvements

## 1. Objective
在 `jki` 指令中新增 `--force-agent` 旗標，並在非靜音模式下顯示 OTP 的獲取來源。

## 2. Tasks
- [x] **CLI Argument**:
    - 在 `crates/jki/src/main.rs` 的 `Cli` 結構中新增 `force_agent: bool` 旗標。
- [x] **Logic Update**:
    - 修改 `run` 函式：若 `force_agent` 為 true，跳過第一階段的 Plaintext (零延遲) 檢查，強制嘗試 Agent 或 Local 解密。
- [x] **UX Refinement**:
    - 修改 `handle_otp_output`：新增一個 `source` 參數。
    - 在調用處標註來源：`[Plaintext]`, `[Agent]`, 或 `[Local]`。
    - 顯示格式：`[Source] Selected: AccountName`。
- [x] **Verification**:
    - 確保 `jki --force-agent` 在明文金庫存在時仍優先連接 Agent。
    - 驗證輸出訊息是否包含正確的標籤。

## 3. Deliverables
- [x] 修改後的 `crates/jki/src/main.rs`。
- [x] 驗證報告 `missions/mission-jki-force-agent-report.md`。
