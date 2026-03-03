# Mission: Secure Export Implementation (jkim export)

## 1. Objective
實作安全匯出功能，將金庫內容轉換為 `otpauth` URI 格式，並封裝至受密碼保護的 ZIP 加密檔中。

## 2. Tasks
- [x] **Dependency Update**:
    - 在 `crates/jkim/Cargo.toml` 中加入 `zip = { version = "2.1", features = ["aes-crypto"] }`。
- [x] **Core Logic (jki-core)**:
    - 為 `Account` 結構實作 `to_otpauth_uri(&self) -> String`。
    - 格式參考：`otpauth://totp/{Issuer}:{Name}?secret={Secret}&issuer={Issuer}&digits={Digits}&algorithm={Algorithm}`。
- [x] **CLI Implementation (jkim)**:
    - 在 `Commands` enum 中新增 `Export { output: Option<PathBuf> }` 子指令。
    - 實作流程：
        1. 驗證 Master Key 並整合 Metadata 與 Secrets。
        2. 提示使用者輸入「匯出密碼」 (Export Password)。
        3. 建立 `export_yyyyMMdd_HHmm.zip`。
        4. 將所有帳戶的 `otpauth` URI 寫入 ZIP 內的 `accounts.txt`。
        5. 使用 AES-256 加密 ZIP 內容。
- [x] **Verification**:
    - 撰寫測試驗證 `otpauth` URI 產生的正確性。
    - 手動測試產出的 ZIP 能否解密。

## 3. Deliverables
- [x] 修改後的 `jki-core` 與 `jkim`。
- [x] 驗證報告 `missions/mission-export-logic-report.md`。
