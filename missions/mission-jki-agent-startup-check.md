# Mission: jki-agent Startup Validation

## 1. Objective
確保 `jki-agent` 在啟動時根據旗標進行環境驗證。特別是在 `--force-age` 模式下，若加密金庫缺失應立即終止，而非進入監聽狀態。

## 2. Tasks
- [ ] **Startup Pre-flight Check (`crates/jki-agent/src/main.rs`)**:
    - 在 `main` 函式的 `LocalSocketListener::bind` 之前加入檢查。
    - 若 `args.force_age` 為 true 且 `JkiPath::secrets_path()` (即 `.age`) 不存在：
        - 輸出精確錯誤：「CRITICAL: Force-age mode enabled but encrypted vault (.age) is missing. Exit.」
        - 呼叫 `process::exit(1)`。
- [ ] **Verification**:
    - 模擬環境：刪除 `.age` 僅保留 `.json`。
    - 測試：執行 `jki-agent --force-age`。
    - 預期結果：程式應立即報錯並結束，不顯示 "listening on..." 訊息。

## 3. Deliverables
- [ ] 修改後的 `jki-agent/src/main.rs`。
- [ ] 驗證報告 `missions/mission-jki-agent-startup-check-report.md`。
