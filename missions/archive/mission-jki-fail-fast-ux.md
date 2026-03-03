# Mission: jki Fail-Fast Logic & Precise Diagnostics

## 1. Objective
落實 Fail-Fast 原則，確保 `jki` 在 Agent 失效或檔案缺失時能給予精確的錯誤提示，而非默默回退或 Panic。

## 2. Tasks
- [ ] **Strict Agent Mode**:
    - 修改 `crates/jki/src/main.rs`：若 `cli.force_agent` 為 true，在 Agent 路徑失敗後，**不應**進入 Local 解密路徑。
- [ ] **Error Propagation**:
    - 在 `jki` 呼叫 Agent `Unlock` 失敗時，擷取 `Response::Error(msg)` 並以 `eprintln!("Error: Agent failed to unlock: {}", msg)` 顯示。
- [ ] **Panic Removal (Local Path)**:
    - 將 `fs::read(&sec_path).expect(...)` 改為友善的錯誤處理。若 `.age` 不存在且無明文金庫，告知使用者：「金庫檔案遺失，請執行 jkim init 或恢復備份。」
- [ ] **Agent Plaintext Support**:
    - 修改 `crates/jki-agent/src/main.rs`：讓 `State::unlock` 支援在 `.age` 缺失時讀取 `.json` 作為備選，但回傳一個包含加載來源的訊息（或透過 Response 告知）。
- [ ] **Verification**:
    - 測試 1：刪除 `.age` 並執行 `jki --force-agent`。預期：Agent 回報找不到檔案，`jki` 顯示該錯誤並直接結束，不發生 Panic。
    - 測試 2：在無任何金庫檔案時執行 `jki`。預期：顯示「金庫檔案遺失」的友善提示。

## 3. Deliverables
- [ ] 修改後的 `jki` 客戶端與 `jki-agent` 服務端。
- [ ] 驗證報告 `missions/mission-jki-fail-fast-ux-report.md`。
