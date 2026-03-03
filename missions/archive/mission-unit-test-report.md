# Mission Report: Unit Test Coverage Analysis

## 1. Summary
成功安裝並配置 `cargo-tarpaulin` 於 macOS 環境（儘管其主要支援 Linux，但在此環境下運作良好），並將覆蓋率檢測整合至 `Makefile`。
透過修復現有測試 Bug 及新增核心邏輯測試（編輯、同步衝突處理），整體程式碼覆蓋率從初始的約 53% 提升至 **60.04%**。

## 2. Tooling Configuration
- **Tool**: `cargo-tarpaulin` (v0.35.2)
- **Makefile Update**: 新增 `make cov` 指令。
    ```makefile
    cov:
        cargo tarpaulin --workspace --out Html
    ```
- **Output**: 產出 `tarpaulin-report.html`。

## 3. Coverage Data
| Crate | Total Lines | Covered Lines | Coverage % |
| :--- | :--- | :--- | :--- |
| `jki-core` | 291 | 225 | 77.32% |
| `jki` | 192 | 86 | 44.79% |
| `jki-agent` | 99 | 60 | 60.61% |
| `jkim` | 409 | 224 | 54.77% |
| **Workspace Total** | **991** | **595** | **60.04%** |

*註：`jki-core` 各模組細節：`import.rs` (96.5%), `paths.rs` (97.1%), `lib.rs` (73.95%), `keychain.rs` (38.5%)。*

## 4. Improvements Made
- **Bug Fix**: 修復了 `jki` 中 `--force-agent` 旗標在 Agent 失敗時無法正確 Fallback 到本地解密的邏輯，並同步修正了測試案例 `test_run_force_agent_skips_plaintext`。
- **New Tests in `jkim`**:
    - `test_handle_edit`: 透過模擬 `EDITOR` 環境變數，驗證 Metadata 編輯與 JSON 驗證流程。
    - `test_handle_sync_conflict_resolve`: 模擬 Git 衝突情境，驗證自動備份與衝突解決（優先保留本地變更）的邏輯。
- **Agent hardening**: 驗證了 `force-age` 模式下的拒絕明文邏輯。

## 5. Next Steps
- **Keychain Testing**: `keychain.rs` 覆蓋率較低 (38.5%)，主要是 `KeyringStore` (實體系統介面) 難以在 Unit Test 中模擬，未來可考慮引入 Integration Tests。
- **TUI Testing**: `jkim` 未來若引入更複雜的 TUI 互動，需考慮 `ratatui` 的後端模擬測試。

## 6. Closure
任務目標已達成。`Makefile` 已更新，覆蓋率報告已產生，核心邏輯已獲得基礎測試覆蓋。
