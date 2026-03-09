# Mission: 實作 `jki --uri` 旗標

## 1. 目標 (Objective)
在 `jki` 執行器中新增 `-U, --uri` 旗標。當唯一匹配目標帳號時，生成該帳號完整的標準 `otpauth://` URI 並進行輸出與剪貼簿複製，以便使用者進行跨裝置遷移或備份。

## 2. 背景與需求 (Context)
*   **物理事實**：Base32 Secret 僅包含金鑰，而 `otpauth://` URI 包含了發行者 (Issuer)、帳號名稱 (Name) 及演算法參數，是行動裝置 App 識別身分的完整格式。
*   **對稱性**：`jkim add -S` 目前已同時輸出 Secret 與 URI，`jki` 應具備獨立獲取 URI 的能力。
*   **防禦性設計**：
    *   **腳本友善**：配合 `-s` 時應僅輸出純 URI 字串。
    *   **安靜模式**：`-q` 應抑制 stderr 的標籤提示。

## 3. 涉及檔案 (Files Involved)
- `crates/jki/src/main.rs` (核心邏輯與 CLI 參數)
- `docs/jki-cli-spec.md` (同步規格)
- `missions/archive/mission-jki-uri-output-report.md` (物理報表)

## 4. `jki --uri` 規格規劃

### CLI 參數更新 (`crates/jki/src/main.rs`)
*   新增旗標：`#[arg(short = 'U', long = "uri")] pub show_uri: bool`

### 行為邏輯 (Behavioral Logic)
1.  **優先序與衝突**：
    *   若同時指定 `-S` 與 `-U`，系統應依序輸出兩者（或以 `-U` 為主），但在剪貼簿複製上需有明確定義（建議以 `-U` 為優先）。
2.  **認證與解密**：
    *   比照 `-S` 流程，獲取主金鑰後執行本地解密（因 Agent 目前不回傳 URI 原始資訊）。
3.  **輸出行為**：
    *   **標籤**：若無 `-q`，stderr 提示 `[URI] Selected: ...`。
    *   **Stdout**：印出 `acc.to_otpauth_uri()`。
    *   **剪貼簿**：若未帶 `-s`，將完整 URI 複製到剪貼簿。

## 5. 單元測試要求 (Unit Testing Requirements)
*   **`test_args_uri`**: 驗證 CLI 參數解析。
*   **`test_run_uri_stdout`**: 驗證匹配唯一帳號時，正確輸出 `otpauth://` 格式。

## 6. 完工定義 (Definition of Done)
1.  [x] `jki --uri` 實作完成，可正確輸出與複製 URI。
2.  [x] 單元測試全數通過（不觸發 Keychain 授權）。
3.  [x] `docs/jki-cli-spec.md` 已完成規格同步。
