# Mission Report: 實作 `jkim add` 子指令與 TOTP Mock 工具

## 1. 任務執行摘要 (Summary)
成功在 `jkim` 中新增 `add` 子指令，實現了手動新增 OTP 帳號與從 URI 匯入的功能。同時交付了 `scripts/mock_totp_gen.py` 工具，用於生成符合 RFC 6238 規範的 Mock 資料。所有單元測試均已通過，規格文件亦同步更新。

## 2. 物理變動紀錄 (Physical Changes)
*   **`crates/jkim/Cargo.toml`**: 
    *   新增 `atty` (0.2), `uuid` (1.0), `base32` (0.4.0) 依賴。
*   **`crates/jkim/src/lib.rs`**:
    *   新增全域 `--quiet` 旗標。
    *   新增 `Add` 子指令及其參數（`name`, `issuer`, `secret`, `uri`, `force`）。
    *   實作 `handle_add` 核心邏輯，包含 Secret 清理、Base32 校驗、衝突檢測及原子化寫入。
    *   新增 3 項單元測試：`test_handle_add_uri`, `test_handle_add_manual_cleaning`, `test_handle_add_conflict`。
*   **`scripts/mock_totp_gen.py`**:
    *   建立 Python 工具，生成 160-bit 隨機熵的 TOTP 資訊。
*   **`docs/jki-cli-spec.md`**:
    *   新增 `jkim add` 指令說明，並修正後續章節編號。

## 3. 行為校驗 (Behavioral Validation)
### 參數決策矩陣驗證
| 物理情境 | 實作行為 | 驗證狀態 |
| :--- | :--- | :--- |
| **衝突 (Conflict)** | 預設 Error + Exit 1; `-f` 下 Warning + Overwrite Success | 通過 |
| **金鑰清理 (Cleaning)** | 自動 `trim()`, `replace(" ", "")`, `to_uppercase()` | 通過 |
| **歷史洩漏 (History)** | TTY 模式下 `--secret` 會發出警告，但 `-q` 會靜默 | 通過 |
| **Base32 校驗** | 格式不符時發出 Warning + Prompt; `-q` 或 `-f` 則略過校驗/警告 | 通過 |

### 測試執行結果
```bash
running 18 tests
test tests::test_handle_add_uri ... ok
test tests::test_handle_add_manual_cleaning ... ok
test tests::test_handle_add_conflict ... ok
...
test result: ok. 18 passed; 0 failed; 0 ignored; finished in 32.60s
```

## 4. 交付物清單 (Deliverables)
1.  **指令**: `jkim add` (已整合至主程式)
2.  **腳本**: `scripts/mock_totp_gen.py`
3.  **規格**: `docs/jki-cli-spec.md` (Updated)

## 5. 後續建議 (Next Steps)
*   目前 `jkim add` 預設生成 `Standard` 類型的帳號，未來可考慮新增 `--type` 參數以支援 Steam 或 Blizzard 等特殊類型。
*   建議在 `jki-agent` 中增加對 Metadata 變動的檔案監聽 (Inotify)，以實現完全自動化的重載。
