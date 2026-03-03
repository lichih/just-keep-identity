# Mission Report: Secure Export Implementation (jkim export)

## 1. Summary
實作了 `jkim export` 指令，能夠將金庫內容轉換為 `otpauth` URI 格式，並安全地封裝至受 AES-256 加密保護的 ZIP 檔案中。

## 2. Changes
### 2.1 Dependencies
- **jkim**: 加入了 `zip` (version 2.1+, 實際編譯使用 2.4.2) 並啟用 `aes-crypto` 特性。

### 2.2 Core Logic (jki-core)
- 在 `Account` 結構中實作了 `to_otpauth_uri(&self) -> String`。
    - 正確處理了 `Issuer` 與 `Name` 的 URL encoding。
    - 輸出格式符合 `otpauth://totp/{Issuer}:{Name}?secret={Secret}&issuer={Issuer}&digits={Digits}&algorithm={Algorithm}`。
- 增加了 `test_to_otpauth_uri` 單元測試。

### 2.3 CLI Implementation (jkim)
- **Subcommand**: 新增 `export [output]` 指令。
- **Logic**:
    1. 使用 Master Key 解鎖金庫並整合 Secrets 與 Metadata。
    2. 提示使用者輸入並確認「匯出密碼」(Export Password)。
    3. 建立 AES-256 加密的 ZIP 檔案。
    4. 將所有帳戶的 URI 以換行分隔寫入 ZIP 內的 `accounts.txt`。
- **Tests**: 新增 `test_handle_export` 整合測試，驗證 ZIP 產出、加密與內容正確性。

## 3. Verification Result
### 3.1 Automated Tests
- `jki-core`: `test_to_otpauth_uri` PASSED.
- `jkim`: `test_handle_export` PASSED. (驗證了 ZIP 建立、AES 解密與內容讀取)

### 3.2 Manual Test
- 執行 `cargo run -- export`。
- 輸入 Master Key 解鎖。
- 輸入匯出密碼 `test1234`。
- 產出 `export_20260303_1554.zip`。
- 使用 `unzip` 或其他工具測試（需支援 AES 256 解密），可正確還原 `accounts.txt`。

## 4. Conclusion
任務順利完成，代碼已通過單元測試與整合測試驗證。
