# Mission: 實作 `jkim add` 活體產碼握手流程 (Live Handshake)

## 1. 目標 (Objective)
在 `jkim add` 物理寫入金庫前，實作一個動態更新的產碼握手介面。確保使用者能在服務商端完成驗證後再執行存檔，落實「寫入即正確」的物理誠信原則。

## 2. 背景與需求 (Context)
*   **物理事實**：TOTP 金鑰錯誤無法自動修復，必須在入口處 (Ingress) 攔截。
*   **權威矩陣對齊**：本任務嚴格遵循 `docs/jki-cli-spec.md` 章節 1.1 的「授權與抑制矩陣」。
*   **授權原則**：
    - 只有在「已預先授權 (`-f`)」且「要求安靜 (`-q`)」時，始得跳過此握手流程。
    - 其他組合（包含未授權的 `-q`）均須執行互動握手。

## 3. 涉及檔案 (Files Involved)
- `crates/jkim/Cargo.toml`: 新增 `crossterm` 以支援鍵盤監聽與原地渲染。
- `crates/jkim/src/lib.rs`: 核心邏輯重構。
- `docs/jki-cli-spec.md`: 更新指令描述。
- `missions/archive/mission-jkim-add-live-handshake-report.md`: 物理報表。

## 4. 實作規範 (Technical Spec)

### 4.1 握手循環 (Handshake Loop)
1.  **進入守衛 (Guard)**：
    - 若 `!atty::is(Stream::Stdin)`，跳過。
    - 若 `cli.force && cli.quiet`，跳過（已授權且抑制）。
    - 若 `cli.default && cli.quiet`，跳過。
    - 其他情況均進入 Loop。
2.  **動態渲染**：
    - 使用 `crossterm` 或轉義字元，在終端機同一行（或區塊）顯示即時 OTP 與倒數。
3.  **鍵盤響應**：
    - `ENTER` -> 退出 Loop 並執行物理存檔。
    - `CTRL-C` / `ESC` -> 程序立即退出，不執行 `fs::write`。

## 6. 完工定義 (Definition of Done)
1.  [x] `jkim add` 成功實作動態更新的握手畫面。
2.  [x] 驗證符合 SSoT 矩陣：未獲授權時的 `-q` 不會跳過握手。
3.  [x] 驗證符合 SSoT 矩陣：`-f -q` 能順暢靜默執行。
4.  [x] 單元測試通過（不影響自動化測試腳本）。

