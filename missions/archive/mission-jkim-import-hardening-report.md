# Mission Report: jkim 匯入邏輯硬化 (Import Logic Hardening)

## 1. 任務概要 (Overview)
依照 `docs/jki-prd-spec.md` 附錄 A 的規範，重構了 `jkim import-winauth` 的狀態偵測與儲存決策邏輯。本次硬化旨在減少冗餘的 y/n 詢問，並嚴格隔離認證職責。

## 2. 實作變更 (Changes)

### 2.1 決策矩陣 (Decision Matrix Implementation)
重構後的 `handle_import_winauth` 遵循以下邏輯：
- **加密優先 (Encrypted State)**：偵測到 `.age` 檔案時，強制認證。成功後直接更新並加密，達成 **0 詢問**。
- **明文維持 (Plaintext State)**：偵測到 `.json` 且無 Master Key 時，直接更新明文，達成 **0 詢問**。
- **升級路徑 (Upgrade Path)**：偵測到 `.json` 且具備 Master Key 時，主動提示升級 `[y/N]`。
- **初始狀態 (Initial State)**：
    - 有 Key：自動建立加密金庫，**0 詢問**。
    - 無 Key：提示建立明文金庫 `[y/n]`。
- **損壞檢查 (Corruption Check)**：偵測到 `metadata` 但 Secrets 物理檔案全失時，直接報錯停止執行，防止降級風險。

### 2.2 代碼清理
- 移除了舊有的 `has_master_key` 物理檔案偵測邏輯，改為依據 `.age` 狀態與 `acquire_master_key` 的返回結果動態決策。
- 修復了 `key_path` 變數未使用的警告。
- 更新了測試案例 `test_import_hardening_logic` 以覆蓋上述 5 種關鍵情境。

## 3. 驗證結果 (Validation)
- **單元測試**：執行 `cargo test -p jkim`，所有測試（包含新增的 hardening 測試）均通過。
- **物理編譯**：執行 `cargo check` 與 `make` 確認編譯通過且無警告。
- **行為驗證**：在現有加密金庫上匯入帳號時，已實現 0 詢問直接更新。

## 4. 結論 (Conclusion)
已完成所有硬化任務，`jkim` 匯入行為現在更符合 JKI 的極速哲學，並具備更高的安全性。
