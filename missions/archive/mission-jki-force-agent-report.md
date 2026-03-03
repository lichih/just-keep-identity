# Mission Report: jki --force-agent & UX Improvements

## 1. Summary
成功在 `jki` 指令中新增 `--force-agent` 旗標，並優化了 OTP 輸出的 UX，現在會顯示金鑰來源（Plaintext, Agent, 或 Local）。

## 2. Changes
### 2.1 crates/jki/src/main.rs
- **CLI Struct**: 新增 `force_agent: bool` 旗標。
- **handle_otp_output**:
    - 新增 `source: &str` 參數。
    - 在非靜音模式下，輸出格式改為 `[Source] Selected: AccountName`。
- **run Function**:
    - 若 `force_agent` 為 true，跳過 `Plaintext` 路徑，強制進入 `Agent` 或 `Local` 路徑。
    - 為各個調用處正確標註來源：`"Plaintext"`, `"Agent"`, `"Local"`。
- **Tests**:
    - 更新 `test_run_full_flow` 以適應 `Cli` 結構的變更。
    - 新增 `test_run_force_agent_skips_plaintext` 測試，驗證 `--force-agent` 確實能跳過明文金庫。

## 3. Verification Results
### 3.1 Unit Tests
執行 `cargo test -p jki`，所有 5 個測試案例均通過：
- `tests::test_handle_agent_with_stream`: OK
- `tests::test_args_parsing`: OK
- `tests::test_args_stdout_short`: OK
- `tests::test_run_full_flow`: OK
- `tests::test_run_force_agent_skips_plaintext`: OK (驗證了 `--force-agent` 的邏輯)

### 3.2 Manual Verification (Simulated)
- 當 `force_agent = false` 且明文金庫存在時，輸出 `[Plaintext] Selected: ...`。
- 當 `force_agent = true` 且明文金庫存在時，跳過明文並嘗試 Agent（若失敗則回退到 Local），輸出 `[Agent] Selected: ...` 或 `[Local] Selected: ...`。

## 4. Conclusion
任務已完成，程式碼符合 PRD 與 Mission Spec 要求。
