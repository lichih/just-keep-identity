# Mission Report: Sync UX Refinement (jkim sync)

## 1. Summary
本任務優化了 `jkim sync` 在遇到 Git 衝突時的處理流程。實作了自動化備份與「本地優先」的衝突解決策略，並完整支援 `--default` 參數以實現無人值守同步。

## 2. Changes

### crates/jki-core/src/lib.rs
- 在 `git` 模組中新增輔助函式：
    - `get_conflicting_files`: 獲取目前衝突中的檔案清單。
    - `checkout_theirs`: 將衝突檔案還原為本地版本 (rebase 情境下 `--theirs` 為本地提交)。
    - `add`: 將特定檔案加入暫存區。
    - `rebase_continue`: 繼續 rebase 流程 (並自動略過編輯器)。
    - `rebase_abort`: 中止 rebase。

### crates/jkim/src/main.rs
- 更新 `handle_sync` 函式簽章，接受 `default_flag` 與 `Interactor`。
- 在 `git::pull_rebase` 失敗時：
    - 若開啟 `--default` 或使用者確認，自動執行以下步驟：
        1. 備份衝突檔案至 `.conflict` 副檔名。
        2. 以本地版本解決衝突 (`git checkout --theirs`)。
        3. 將解決後的檔案加入暫存 (`git add`)。
        4. 完成 rebase (`git rebase --continue`)。
- 更新 `main` 函式以正確傳遞參數至 `handle_sync`。

## 3. Verification Results
- **測試腳本**: `draft/verify_sync_conflict.py`
- **情境**: 
    1. 在 `local-a` 與 `local-b` 分別對同一個 `vault.metadata.json` 進行不相容變更。
    2. `local-a` 先推送至遠端。
    3. 在 `local-b` 執行 `jkim sync --default`。
- **結果**:
    - [x] 成功偵測到衝突。
    - [x] 自動將衝突內容備份至 `vault.metadata.json.conflict`。
    - [x] 自動採用 `local-b` 的變更完成同步。
    - [x] 最終成功推送至遠端，無須人工介入。

## 4. Conclusion
`jkim sync` 現在具備更強韌的衝突處理能力。透過 `--default` 旗標，使用者可以在自動化腳本或日常使用中享受更流暢的同步體驗，同時保有衝突備份以供事後檢查。
