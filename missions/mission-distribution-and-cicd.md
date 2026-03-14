# Mission: 專業發布自動化與分發渠道 (Professional Distribution & CI/CD)

## 🎯 目標 (Goals)
建立自動化的發布管線，讓 JKI 支援一鍵安裝（如 Homebrew）與跨平台（macOS/Linux/Windows）二進位檔自動分發，降低使用者的進入門檻。

---

## 📝 任務清單 (Tasks)

### Phase 1: 發布自動化 (Release Automation)
- [ ] **導入 cargo-release**：參照 `mission-automated-versioning.md` 實現自動版號管理。
- [ ] **引入 cargo-dist**：配置 GitHub Actions 流程，實現打 Tag 後自動編譯。
- [ ] **安裝腳本自動化**：生成支援 `curl | sh` 的快速安裝腳本。

### Phase 2: Homebrew 生態對接 (Homebrew Ecosystem)
- [x] **建立專屬 Tap**：已建立 `lichih/homebrew-jki` 並完成初始同步。
- [ ] **自動化 Formula 更新**：配置 CI 流程，每當新版本發布時自動更新。
- [x] **驗證安裝**：已完成。透過 `docs/homebrew-test-guide.md` 驗證了隔離環境下的 Tap 安裝。

### Phase 3: 多平台分級打包 (Tiered Packaging)
- [x] **認證鏈自動化 (macOS)**：已整合 `make sign-bins` 與 `make dist-macos`。
- [x] **安裝腳本優化**：`install.sh` 已支援 `--skip-build` 以保留簽名。
- [ ] **CLI-Only 發布 (Win/Linux)**：優化編譯配置，為非 macOS 平台產出輕量化、無 Agent 的純 CLI 二進位檔。
- [ ] **Windows 信任度解決方案**：研究如何在無簽名情況下降低 SmartScreen 攔截感。

---

## 📈 成功定義 (Definition of Done)
1.  每當執行 `git tag` 並 push 時，GitHub 會自動產生包含所有平台 binary 的 Release。
2.  使用者可以透過 `brew install` 安裝 JKI。
3.  提供一鍵式的 `curl` 安裝命令。

---

## 📅 歸檔紀錄 (Archive)
- [x] **Mission: v0.1.0-alpha Public Release Prep** (已完成雙語 README、Demo GIF 與專案元數據更新)。
