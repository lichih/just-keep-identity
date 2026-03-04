# Mission: jkim Agent 管理指令與 jki 靜默化 (Agent Control & jki Silence)

## 1. 背景 (Context)
根據最新的「代理服務啟動政策 (附錄 B)」，`jki` 應移除自動啟動 Agent 的副作用，而將該職責移轉至 `jkim agent` 指令下。

## 2. 核心任務 (Tasks)
- [ ] **jki 客戶端重構 (`crates/jki/src/main.rs`)**:
    - [ ] 移除 `run` 函式中呼叫 `ensure_agent_running` 的邏輯。
    - [ ] 實作「被動解鎖」：僅在 `AgentClient::ping()` 為 true 時，才嘗試呼叫 `AgentClient::unlock`。
    - [ ] 實作「引導提示」：當偵測到 Agent 未運行時，在 `stderr` 顯示：`[Tip] Start jki-agent with 'jkim agent start' for faster lookups.`。
- [ ] **jkim 管理端擴充 (`crates/jkim/src/main.rs`)**:
    - [ ] 新增 `Commands::Agent` 子指令結構。
    - [ ] 實作 `handle_agent_start`: 呼叫 `jki_core::ensure_agent_running(false)`。
    - [ ] 實作 `handle_agent_stop`: 透過 `AgentClient` 發送停止請求（或透過 OS 信號）。
- [ ] **物理驗證**:
    - [ ] 執行 `make release` 確保編譯。
    - [ ] 測試 `jki` 查詢時不再自動啟動 Agent。
    - [ ] 測試 `jkim agent start` 能正確啟動背景 Agent 並顯示 Tray Icon。

## 3. 涉及檔案 (Files Involved)
- `crates/jki/src/main.rs`
- `crates/jkim/src/main.rs`
- `missions/mission-jkim-agent-control-report.md` (New)

## 4. 驗收標準 (Exit Criteria)
- [ ] 產出 `missions/mission-jkim-agent-control-report.md`。
- [ ] `jki` 執行時若 Agent 沒開，不應出現 Tray Icon。
- [ ] `jkim agent start` 執行後應出現 Tray Icon。

---
*Status: Defined by Architect. Enforcing Lifecycle Policy.*
