# Mission Report: Agent 可視化整合 (Menu Bar Icon / System Tray)

## 1. 任務概述 (Overview)
本任務成功為 `jki-agent` 實作了 macOS/Windows 上的系統選單列圖示 (Tray Icon)，提供了即時狀態查看、快速上鎖與優雅退出的功能。

## 2. 實作細節 (Implementation Details)

### 2.1 Dependency 變更
在 `crates/jki-agent/Cargo.toml` 中新增以下依賴：
- `tray-icon`: 提供跨平台系統托盤支援。
- `tao`: 輕量級視窗與事件循環庫（用於 macOS/Windows 渲染）。
- `muda`: 跨平台選單建構庫。

### 2.2 架構調整
- **多執行緒運作**: 
    - 將原本阻塞主執行緒的 `LocalSocketListener` 移至獨立執行緒處理。
    - 主執行緒改為運行 `tao` 的 Event Loop，確保 UI 響應性。
- **State 公開化**: 
    - 將 `jki-agent/src/main.rs` 中的 `State` 結構體及其欄位設為 `pub`，以便 `tray` 模組讀取金庫鎖定狀態。
- **新增 Tray 模組**: 
    - 實作 `crates/jki-agent/src/tray.rs`，封裝選單建構、狀態更新與事件處理邏輯。

### 2.3 功能實作
- **Status (選單頂部)**: 顯示 "Status: Locked" 或 "Status: Unlocked"，隨 Agent 內部的 `secrets` 狀態即時變動。
- **Lock (選單項)**: 點擊後會清除內存中的機密 (secrets, master_key, last_unlocked)，達成物理上鎖。
- **Quit (選單項)**: 正常關閉事件循環並終止程序。
- **macOS 優化**: 透過 `ActivationPolicy::Accessory` 隱藏 Dock 圖示，僅保留頂部選單列存在。

## 3. 驗證結果 (Verification)

### 3.1 編譯驗證
- 執行 `cargo check -p jki-agent` 通過。
- 修正了 `event_loop` 可變性錯誤與 Unused Imports。

### 3.2 功能測試
- 執行 `cargo test -p jki-agent` 通過（共 7 個測試），確保 Socket 通訊與加解密核心邏輯未受影響。
- 手動驗證邏輯：Event Loop 正確接收 `MenuEvent` 並觸發相應的狀態變更。

## 4. 交付物 (Deliverables)
- `crates/jki-agent/Cargo.toml` (更新依賴)
- `crates/jki-agent/src/main.rs` (重構主邏輯)
- `crates/jki-agent/src/tray.rs` (新增選單實作)

---
*Status: Completed. Verified on macOS.*
