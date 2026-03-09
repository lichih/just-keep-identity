# Mission: 實作 `jkim add` 子指令與 TOTP Mock 工具

## 1. 目標 (Objective)
在 `jkim` 中新增 `add` 指令，並提供一個符合 RFC 規範的 Mock 生成工具。建立一個尊重使用者主權、符合開發者直覺且「自動化友善」的分錄管理介面。

## 2. 背景與需求 (Context)
*   **物理事實**：TOTP Secret 長且需 Copy-Paste。
*   **自動化障礙**：單元測試與正式環境的 Binary 不同，存取 Keychain 會觸發系統 OS 詢問，破壞自動化流程。
*   **認證優先序修正**：為支援自動化，`jkim` 的認證優先序應調整為：**Master Key File > Agent > Keyring**。

## 3. 涉及檔案 (Files Involved)
- `crates/jkim/src/lib.rs` (核心邏輯)
- `crates/jki-core/src/lib.rs` (Auth 優先序調整)
- `docs/jki-cli-spec.md` (同步規格)
- `scripts/mock_totp_gen.py` (Mock 生成工具)
- `missions/mission-jkim-add-subcommand-report.md` (物理報表)

## 4. `jkim add` 指令規格規劃

### 參數決策矩陣 (Behavioral Matrix)
(與先前討論一致：-q 抑制警告但報錯必達，-f 強制新增不覆蓋)

### 認證與自動化原則 (Automation First)
1.  **Auth Priority**: 實作時應確保 `acquire_master_key` 優先檢查 `$JKI_HOME/master.key`。
2.  **隱蔽輸入**：重用 `TerminalInteractor`。
3.  **清理邏輯**：存入前執行 `trim()`, `replace(" ", "")`, `to_uppercase()`。

## 5. 單元測試要求 (Unit Testing Requirements)
*   **`test_uri_parsing`**
*   **`test_secret_cleaning`**
*   **`test_conflict_matrix`**
*   **`test_auth_priority_automation`**: 確保在存在 `master.key` 的情況下，單元測試能「靜默」通過而不觸發任何 OS 或 Agent 請求。

## 6. Mock 生成工具規格 (`scripts/mock_totp_gen.py`)
(符合 RFC 6238 規範)

## 7. 完工定義 (Definition of Done)
1.  [x] `jkim add` 實作完成。
2.  [x] **單元測試全數通過（且不觸發 Keychain 授權視窗）**。
3.  [x] `docs/jki-cli-spec.md` 已完成規格同步。
