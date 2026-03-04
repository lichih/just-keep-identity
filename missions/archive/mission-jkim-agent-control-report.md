# Mission Report: jkim Agent 管理指令與 jki 靜默化 (Agent Control & jki Silence)

## 1. 執行摘要 (Executive Summary)
本任務已成功重構 `jki` 與 `jkim`，將背景 Agent (`jki-agent`) 的啟動職責從 `jki` 轉移至 `jkim`。`jki` 現在以「被動方式」與 Agent 互動，若 Agent 未運行，`jki` 將不再自動啟動它，而是回退至本地解密或顯示引導提示。同時，`jkim` 擴充了 `agent` 子指令，支援 `start`、`stop` 與 `reload`。

## 2. 實作變更 (Implementation Changes)

### 2.1 jki-core (`crates/jki-core/src/lib.rs`)
- **新增 `Request::Shutdown`**: 支援透過 Socket 發送關閉請求。
- **擴充 `AgentClient`**: 
    - 實作 `AgentClient::shutdown()` 函式。
    - 實作 `AgentClient::reload()` 函式（發送 `Reload` 請求）。

### 2.2 jki-agent (`crates/jki-agent/src/main.rs`)
- **實作 `Shutdown` 處理**: 
    - 透過 `std::sync::mpsc` 通道將關閉信號傳回主執行緒。
    - 在 `tao` 的 Event Loop 中偵測信號並調用 `ControlFlow::Exit` 正常退出。
    - 確保 Socket 資源能被正確釋放（針對 Unix）。

### 2.3 jki 客戶端 (`crates/jki/src/main.rs`)
- **移除自動啟動**: 移除 `run` 函式與 `handle_agent` 中呼叫 `ensure_agent_running` 的邏輯。
- **被動解鎖 (Passive Unlock)**: 
    - 在查詢流程中，僅在 `AgentClient::ping()` 為 true 時才嘗試連接 Agent。
    - 若 Agent 未運行且使用者顯式指定 `-A agent` 或 `-A biometric`，則顯示引導提示：`[Tip] Start jki-agent with 'jkim agent start' for faster lookups.`。
- **被動同步**: 當執行本地解密成功後，僅在 Agent 已運行時才嘗試同步（Sync/Unlock）至 Agent，不主動啟動。

### 2.4 jkim 管理端 (`crates/jkim/src/main.rs`)
- **新增 `agent` 指令**:
    - `jkim agent start`: 啟動背景 Agent（調用 `jki_core::ensure_agent_running(false)`）。
    - `jkim agent stop`: 發送 `Shutdown` 請求至 Agent。
    - `jkim agent reload`: 發送 `Reload` 請求至 Agent 以清空快取的 secrets。

## 3. 驗證結果 (Validation)

### 3.1 編譯驗證
- 執行 `make release` 通過，無錯誤（已修復 `jki` 中未使用的 import 警告）。

### 3.2 功能測試
- **靜默化測試**: 在 Agent 未啟動時執行 `jki google`，顯示 `Falling back to local decryption...`，未自動啟動 Agent (無 Tray Icon)。
- **引導提示測試**: 在 Agent 未啟動時執行 `jki -A agent google`，顯示 `[Tip] Start jki-agent with 'jkim agent start' for faster lookups.` 並以退出碼 1 結束。
- **jkim 控制測試**:
    - 執行 `jkim agent start`：成功啟動背景 Agent。
    - 執行 `jkim agent status`：顯示 `jki-agent : Running (Locked)`。
    - 執行 `jki google`：偵測到 Agent 已運行，成功自動解鎖並同步，`status` 變為 `Unlocked`。
    - 執行 `jkim agent stop`：Agent 接收請求後正常退出，`ps aux` 確認進程消失。

## 4. 結論 (Conclusion)
任務目標已達成。`jki` 現在更符合 CLI 工具的「簡潔、無副作用」原則，而 Agent 的管理則集中在 `jkim` 中，提供了更清晰的職責劃分。

---
*Status: Completed by Gemini CLI. Verification Passed.*
