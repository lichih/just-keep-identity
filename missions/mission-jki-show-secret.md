# Mission: 實作 `jki --show-secret` 旗標

## 1. 目標 (Objective)
在 `jki` 執行器中新增 `-S, --show-secret` 旗標。當唯一匹配目標帳號時，不生成 OTP 碼，而是直接輸出儲存的原始 Base32 Secret，以利開發者進行除錯與實體驗證。

## 2. 背景與需求 (Context)
*   **物理事實**：當 TOTP 產碼結果與預期不符（如伺服器拒絕登入）時，開發者需要直接核對物理金鑰字串。
*   **現狀缺口**：目前需依賴 `jkim decrypt` 或 `export` 才能查看底層金鑰，流程繁瑣且有檔案殘留風險。
*   **防禦性設計**：`--show-secret` 應為顯式請求，預設依然是產碼。顯示金鑰前需通過標準認證流程（符合 `GEMINI.md` 的 Auth 優先序）。

## 3. 涉及檔案 (Files Involved)
- `crates/jki/src/main.rs` (核心邏輯與 CLI 參數)
- `docs/jki-cli-spec.md` (同步規格)
- `missions/mission-jki-show-secret-report.md` (物理報表)

## 4. `jki --show-secret` 規格規劃

### CLI 參數更新 (`crates/jki/src/main.rs`)
*   新增旗標：`#[arg(short = 'S', long = "show-secret")] pub show_secret: bool`

### 行為邏輯 (Behavioral Logic)
1.  **衝突互斥**：若 `--show-secret` 與 `--list` (`-l`) 或 `--otp` (`-o`) 同時使用，CLI 應報錯或以 `--show-secret` 為最高優先（針對唯一匹配的目標）。
2.  **查詢與認證**：
    *   重用現有的 `resolve_target` 尋找目標帳號。
    *   若結果不唯一，顯示歧義清單（與現有邏輯一致）。
    *   若結果唯一，觸發金庫解鎖（Agent / Keyfile / Interactive）。
3.  **輸出行為**：
    *   成功解鎖後，**不呼叫 `generate_otp`**。
    *   直接輸出該帳號的 `Secret`。
    *   **安靜模式 (`-q`)**：若無 `-q`，stderr 可輸出提示 `[Secret] Selected: ...`；若有 `-q`，則僅在 stdout 輸出純金鑰字串。
    *   **剪貼簿行為**：若未帶 `--stdout` (`-s`)，則將 Secret 複製到剪貼簿，並在 stderr 提示（與現有 OTP 行為一致）。

### 與專案憲法對齊 (`GEMINI.md`)
*   本任務之實作邏輯必須嚴格遵循專案根目錄 `GEMINI.md` 中關於「認證優先序 (File > Agent)」及「CLI 防禦性設計」之規範。
*   必須包含單元測試，且單元測試在存在 `master.key` 時不可觸發 OS 授權視窗。

## 5. 單元測試要求 (Unit Testing Requirements)
在 `crates/jki/src/main.rs` 或相關測試模組中實作：
*   **`test_args_show_secret`**: 驗證 CLI 參數解析正確。
*   **`test_run_show_secret_stdout`**: 驗證帶有 `--show-secret` 且匹配唯一帳號時，stdout 正確印出金鑰而非 OTP。

## 6. 完工定義 (Definition of Done)
1.  [x] `jki --show-secret` 實作完成，可正確輸出原始金鑰。
2.  [x] 單元測試全數通過（不觸發 Keychain 授權）。
3.  [x] `docs/jki-cli-spec.md` 已完成規格同步。
4.  [x] 產出報表並清理 `.stable` 檔案。
