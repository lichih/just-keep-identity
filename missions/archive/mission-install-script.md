# Mission: Installation Script & UX Improvements

## 1. Objective
提供自動化安裝腳本 `install.sh` 並優化 `Makefile`，讓使用者能一鍵完成編譯、安裝與 PATH 設置。

## 2. Tasks
- [x] **Create `install.sh`**:
    - [x] 檢查 Rust 環境。
    - [x] 執行 `cargo build --release`。
    - [x] 將執行檔安裝至 `~/.local/bin` 或 `~/bin`。
    - [x] **PATH 檢測**：若安裝路徑不在 PATH 中，詢問使用者是否自動加入至 `.zshrc` 或 `.bashrc`。
    - [x] 支援「靜默模式」(Silent Mode) 用於 CI/CD。
- [x] **Enhance `Makefile`**:
    - [x] 新增 `dev` target：啟動 `cargo watch` 或編譯 debug 版本。
    - [x] 新增 `test-all` target：執行所有 crate 的測試。
    - [x] 連結 `make install` 到 `install.sh` 或維持原有邏輯但提供提示。
- [x] **Documentation**:
    - [x] 更新 `README.md` 中的安裝指南，引導使用者使用 `curl | sh` 式的安裝流程（本地模擬）。

## 3. Deliverables
- [x] `install.sh` 腳本。
- [x] 優化後的 `Makefile`。
- [x] 驗證報告 `missions/mission-install-script-report.md`。
