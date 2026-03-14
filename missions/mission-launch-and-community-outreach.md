# Mission: JKI Launch & Community Outreach (Showcase Strategy)

## 🎯 目標 (Goals)
將 JKI 從開發階段推向全球開源與安全社群。不僅是展示一個工具，更是展示一套關於「極致速度」、「物理安全性」與「現代工程品質」的工程理念。

---

## 🚀 核心宣傳亮點 (The Core Hooks)

1.  **Velocity (極致速度)**：
    - 查詢與產碼全流程極速反應。
    - 專為終端機重度使用者設計的 "Micro-Roll" 一手操作邏輯。
2.  **Physical Integrity (物理安全性)**：
    - **Local-first**：秘密僅存在於 OS 原生 Keyring (macOS Keychain / Linux Secret Service)，絕對拒絕雲端。
    - **Zero Disk Leak**：秘密永遠不會以明文形式寫入磁碟。
3.  **Modern Onboarding (專業發布流程)**：
    - **Official Distribution**：提供經過官方簽名與公證的 macOS 套件，避免 Gatekeeper 警告。
    - **Verified Pipeline**：高測試覆蓋率（>65%）確保核心安全路徑的絕對穩定。

---

## 📝 Hacker News (Show HN) 計畫

### 建議標題 (Suggested Titles)
- `Show HN: JKI – A low-friction MFA manager for CLI power users`
- `Show HN: JKI – Local-first terminal 2FA built in Rust with OS Keyring integration`

### 第一則評論草稿 (Introductory Comment)
> "I built JKI because I was frustrated with the friction of traditional 2FA apps. As a developer, I live in the terminal, and I wanted my identity tokens to be as fast as my aliases.
> 
> Key features:
> - **Speed**: Search and copy with minimal keystrokes.
> - **Privacy**: It leverages your OS native keychain. No cloud, no proprietary sync.
> - **Distribution**: Officially signed and notarized for macOS. Available via Homebrew.
> 
> Would love to hear your thoughts on the UX and the local-first approach!"

---

## 🤖 Reddit 分享策略

### r/rust (技術深度)
- **焦點**：分享如何使用 Trait 系統達成 90% 以上的 Keychain 測試覆蓋率。
- **亮點**：Rust + IPC + Security Audit Scripts。

### r/commandline (人體工學)
- **焦點**：展示模糊搜尋的高亮效果與 `jki [pattern] [index]` 的快速操作手感。
- **資產**：`docs/assets/demo.gif`。

---

## 🖼 視覺與驗證資產 (Assets Checklist)
- [ ] **Demo GIF**：確保展示了模糊匹配高亮與複製成功的提示。
- [ ] **Homebrew Verify**：提供 `brew tap lichih/jki` 的一鍵安裝說明。
- [ ] **Signature Check**：展示 `codesign -dvvv` 的官方認證結果以建立信任。

---

## 📈 回饋追蹤 (Feedback Loop)
- 預期回饋量：> 20 則有效建議。
- 監控 Issue 標籤：`community-feedback`。

**狀態：準備發布 (Ready for Launch)**
