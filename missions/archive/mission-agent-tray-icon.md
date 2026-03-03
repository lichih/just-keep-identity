# Mission: Agent 可視化整合 (Menu Bar Icon / System Tray)

## 1. 背景 (Context)
根據 V28 規格，`jki-agent` 在 macOS/Windows 上應具備選單列圖示 (Tray Icon)，供使用者實時查看金庫狀態並進行「快速上鎖 (Lock)」與「優雅退出 (Quit)」。

## 2. 核心任務 (Tasks)
- [ ] **Dependency 整合**:
    - [ ] 在 `jki-agent/Cargo.toml` 中新增 `tray-icon`, `tao`, `menu-item`。
    - [ ] 定義 `ui` feature，並在 macOS/Windows 上預設啟用。
- [ ] **架構重構 (main.rs)**:
    - [ ] 將 `LocalSocketListener` 監聽邏輯搬移到獨立執行緒 (`std::thread`)。
    - [ ] 主執行緒啟動 `tao` Event Loop 負責 UI 渲染。
- [ ] **選單功能實作**:
    - [ ] `Status`: 實時顯示 "Locked" 或 "Unlocked" (與 `State` 連動)。
    - [ ] `Lock`: 點擊後發送請求清除 `State` 記憶體中的機密。
    - [ ] `Quit`: 優雅關閉程序並清理 Socket。
- [ ] **macOS 優化**:
    - [ ] 配置 `LSUIElement`，實現隱身 Dock 僅在 Menu Bar 運行的功能。

## 3. 涉及檔案 (Files Involved)
- `crates/jki-agent/Cargo.toml`
- `crates/jki-agent/src/main.rs`
- `crates/jki-agent/src/tray.rs` (新模組)

---
*Status: Delegated by Architect. UI/Icon Focus.*
