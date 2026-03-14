# Mission: 專業發布自動化與分發渠道 (Professional Distribution & CI/CD)

## 🎯 目標 (Goals)
建立自動化的發布管線，讓 JKI 支援一鍵安裝（如 Homebrew）與跨平台（macOS/Linux/Windows）二進位檔自動分發，降低使用者的進入門檻。

---

## 📝 任務清單 (Tasks)

### Phase 1: 發布自動化 (Release Automation)
- [ ] **引入 cargo-dist**：配置 `cargo-dist` 以生成 GitHub Actions 流程，實現打 Tag 後自動編譯與上傳 Release Assets。
- [ ] **跨平台編譯校驗**：在 GitHub Actions 中驗證 Linux 與 Windows 的編譯正確性。
- [ ] **安裝腳本自動化**：生成支援 `curl | sh` 的快速安裝腳本。

### Phase 2: Homebrew 生態對接 (Homebrew Ecosystem)
- [ ] **建立專屬 Tap**：建立 `lichih/homebrew-jki` 儲存庫。
- [ ] **自動化 Formula 更新**：配置 CI 流程，每當新版本發布時自動更新 brew 用的 Ruby 腳本。
- [ ] **驗證安裝**：測試 `brew install lichih/jki/jki` 的流暢度。

### Phase 3: 多平台打包優化 (Platform Packaging)
- [ ] **macOS App Bundle 簽名**：完善 `make sign` 流程，確保 `jki-agent.app` 在其他 Mac 上不會被 Gatekeeper 阻擋。
- [ ] **Windows 便攜版**：針對 Windows 生成不需要安裝的 `.zip` 包。

---

## 📈 成功定義 (Definition of Done)
1.  每當執行 `git tag` 並 push 時，GitHub 會自動產生包含所有平台 binary 的 Release。
2.  使用者可以透過 `brew install` 安裝 JKI。
3.  提供一鍵式的 `curl` 安裝命令。

---

## 📅 歸檔紀錄 (Archive)
- [x] **Mission: v0.1.0-alpha Public Release Prep** (已完成雙語 README、Demo GIF 與專案元數據更新)。
