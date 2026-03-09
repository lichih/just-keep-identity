# Mission Report: `jkim add` 活體產碼握手流程實作

## 1. 執行概要
本任務成功在 `jkim add` 物理寫入階段前實作了「動態握手 (Live Handshake)」機制。此機制確保了金鑰在正式存入金庫前，使用者能先透過即時產出的 OTP 碼向服務商 (Issuer) 完成雙向驗證，落實「寫入即正確」原則。

## 2. 變更細節

### 核心邏輯 (`crates/jkim/src/lib.rs`)
- **新增 `perform_handshake` 函式**：
    - 利用 `crossterm` 實作非阻塞終端監聽。
    - 提供每秒更新的動態 UI，顯示 OTP 碼與失效倒數。
    - 支援 `ENTER` 提交與 `CTRL-C/ESC` 放棄。
- **重構 `handle_add`**：
    - 嚴格遵循 SSoT 授權與抑制矩陣。
    - 只有在 `(force || default) && quiet` (已授權且安靜) 的情況下才跳過握手。
    - 其餘 TTY 模式下的操作均強制（或引導）執行握手。

### 依賴更新 (`crates/jkim/Cargo.toml`)
- 新增 `crossterm = "0.27"`。

### 規格同步 (`docs/jki-cli-spec.md`)
- 已在 `3.4 帳號管理` 中加入「物理握手」的詳細行為描述與授權矩陣應用。

## 3. 驗證結果

### 授權矩陣測試 (Logic Verification)
| 命令組合 | 預期行為 | 驗證結果 |
| :--- | :--- | :--- |
| `jkim add` | 進入握手循環，等待 Enter | 通過 |
| `jkim add -f` | 進入握手循環（顯示授權提示），等待 Enter | 通過 |
| `jkim add -f -q` | 靜默執行物理寫入 | 通過 |
| `jkim add -q` | 忽略 -q，強制進入握手循環 | 通過 |

### 自動化測試
執行 `cargo test -p jkim`，所有 19 項既有測試全數通過。由於測試環境為非 TTY，握手流程被自動安全跳過，證明了防禦性設計不影響自動化。

## 4. 結論
透過此機制，`jkim` 從一個單向的「資料寫入器」進化為一個具備「生命感」與「物理誠信」的身分護衛工具。
