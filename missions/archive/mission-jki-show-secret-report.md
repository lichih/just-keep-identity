# Mission Report: `jki --show-secret` 旗標實作

## 1. 執行概要
本任務已成功在 `jki` 執行器中新增 `-S, --show-secret` 旗標。當搜尋結果為唯一匹配時，系統會跳過 OTP 產碼流程，直接解鎖金庫並輸出原始的 Base32 Secret。

## 2. 變更細節

### 核心邏輯 (`crates/jki/src/main.rs`)
- **CLI 參數**: 新增 `show_secret: bool` 旗標。
- **輸出重構**: 將 `handle_otp_output` 重構為更通用的 `handle_output`，支援 "OTP" 與 "Secret" 兩種資料類型，並能動態調整 stderr 的提示文字。
- **認證流程**: 
    - 嚴格遵守 `GEMINI.md` 的認證優先序（Plaintext > Agent > Keyfile > Interactive）。
    - 若指定 `--show-secret`，即使 Agent 在運行，也會主動透過 `AgentClient::get_master_key()` 獲取主金鑰進行本地解密，以獲取完整的 Secret 字串（因 Agent IPC 目前僅曝露 OTP）。
- **Bug 修復**: 修正了 `resolve_target` 在使用序號 (Index) 選擇時，返回的匹配列表長度不正確的問題，使之與既有單元測試預期一致。

### 規格同步 (`docs/jki-cli-spec.md`)
- 已在 `2.1 搜尋與 OTP 生成` 章節新增 `-S, --show-secret` 旗標說明。

## 3. 驗證結果

### 單元測試
執行 `cargo test -p jki`，所有 10 項測試全數通過：
- `tests::test_args_show_secret`: 驗證參數解析。
- `tests::test_run_show_secret_stdout`: 驗證 `--show-secret` 執行路徑。
- `tests::test_resolve_target_index_simple`: 驗證序號選擇邏輯與結果集完整性（已修復）。
- `tests::test_run_full_flow`: 驗證標準產碼流程未受影響。

### 物理驗證 (手動測試)
1. `jki <pattern> -S`: 成功顯示 `[Secret] Selected: ...` 並將金鑰複製到剪貼簿。
2. `jki <pattern> -S -s`: 成功在 stdout 印出純文字金鑰。
3. `jki <pattern> -S -q -s`: 僅輸出金鑰，無任何 stderr 提示。

## 4. 結論
`jki --show-secret` 提供了開發者核對底層金鑰的捷徑，同時維持了原有的安全認證機制與 CLI UX 一致性。任務圓滿完成。
