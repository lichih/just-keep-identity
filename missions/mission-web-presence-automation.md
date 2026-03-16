# Mission: JKI Web Presence & Automation

## 🎯 目標 (Goals)
建立 `jki.4649.tw` 作為官方產品入口，並達成「代碼隨行、Git 分離」的自動化部署流程。

---

## 🏗 技術架構 (Architecture)
1. **Source**: 主專案目錄 `/website`。
2. **Distribution**: 獨立倉庫 `lichih/jki.4649.tw` (GitHub)。
3. **Hosting**: Cloudflare Pages。
4. **Automation**: `make publish-site` 指令。

---

## ✨ 網頁規格 (The "Honest" Spec)
- **視覺**：極簡 Terminal 風格 (黑底、高對比白/綠字)。
- **核心內容**：
    - `brew install` 指令框。
    - `demo.gif` 視覺展示。
    - **"The Backstory"**：誠實的開發初衷。
- **SEO**：
    - JSON-LD (`SoftwareApplication`).
    - Meta Tags (`2FA`, `MFA`, `Rust`, `macOS Keychain`).

---

## 🤖 自動化部署 (Makefile)
- [x] 實作 `make publish-site`。
- [x] 確保使用臨時工作區，避免污染主專案 Git。
- [x] 支援 Force Push 到 `jki.4649.tw` 倉庫。

---

## ✅ 驗證清單 (Checklist)
- [x] `website/index.html` 內容完成。
- [x] `Makefile` 指令測試通過 (已成功部署至 Private Repo)。
- [ ] Cloudflare Pages 成功對接新 Repo。
- [ ] `jki.4649.tw` 域名解析完成並開啟 Web Analytics。

**狀態：執行中 (In Progress)**
