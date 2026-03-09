# Mission: 強化 `jkim add` 功能與安全性回饋

## 1. 目標 (Objective)
在 `jkim add` 指令中新增 `-S, --show-secret` 旗標。當成功新增或覆蓋帳號後，直接在終端輸出該帳號的原始 Base32 Secret 與 OTPAuth URI，以便使用者立即進行實體核對或備份。

## 2. 背景與需求 (Context)
*   **物理事實**：使用者在 `add` 完畢後，往往需要立即確認金鑰是否正確（例如手動輸入時）。
*   **一致性**：對齊 `jki --show-secret` 的行為模式，提供對稱的除錯與驗證工具。
*   **防禦性設計**：輸出金鑰時應遵守 `--quiet` 規範。若有 `-q`，則僅輸出金鑰字串；若無 `-q`，則提供標籤提示。

## 3. 涉及檔案 (Files Involved)
- `crates/jkim/src/lib.rs` (核心邏輯與 CLI 參數)
- `docs/jki-cli-spec.md` (同步規格)

## 4. `jkim add --show-secret` 規格規劃

### CLI 參數更新 (`crates/jkim/src/lib.rs`)
*   新增旗標：`#[arg(short = 'S', long = "show-secret")] pub show_secret: bool`

### 行為邏輯 (Behavioral Logic)
1.  **新增流程**：維持現有的 `handle_add` 流程（認證、清理、寫入）。
2.  **成功後置動作**：
    *   若 `--show-secret` 為真：
        *   從記憶體中的 `acc` 物件提取 `secret`。
        *   若無 `-q`，stderr 輸出 `[Secret] Added: ...`。
        *   stdout 輸出原始 Base32 Secret。
        *   stdout (下一行) 輸出 `acc.to_otpauth_uri()`。
    *   若未帶 `--show-secret`：維持現有行為（僅提示 `Account added successfully`）。

## 5. 單元測試要求 (Unit Testing Requirements)
*   **`test_handle_add_show_secret`**: 驗證帶有 `--show-secret` 時，函式執行成功且邏輯分支正確觸發。

## 6. 完工定義 (Definition of Done)
1.  [x] `jkim add --show-secret` 實作完成。
2.  [x] 單元測試通過。
3.  [x] `docs/jki-cli-spec.md` 已完成規格同步。

