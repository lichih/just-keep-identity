# Mission: Sync UX Refinement (jkim sync)

## 1. Objective
優化 `jkim sync` 的衝突處理流程，利用 `confirm(..., default=true)` 實作自動化建議路徑。

## 2. Tasks
- [ ] **Conflict Strategy Implementation**:
    - 在 `git::pull_rebase` 失敗時，利用 `Interactor` 詢問使用者處置策略。
    - **預設策略 (Default Flag)**：若 `--default` 開啟，預設執行「備份衝突並以本地為主」或「自動嘗試 rebase」。
    - 具體動作：若發生衝突且 `default=true`，將衝突檔案暫存後完成 rebase。
- [ ] **Interactor Support**:
    - 檢查 `handle_sync` 邏輯，確保所有互動環節皆支援 `default_flag`。
- [ ] **Verification**:
    - 模擬 Git 衝突情境，測試 `jkim sync --default` 是否能不中斷完成流程。

## 3. Deliverables
- [ ] 修改後的 `jkim` 程式碼。
- [ ] 驗證報告 `missions/mission-sync-ux-report.md`。
