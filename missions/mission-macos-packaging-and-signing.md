# Mission: macOS 官方打包與公證預備 (Packaging & Signing)

## 1. 背景 (Context)
為了提升 JKI 的專業信任度並發揮 Apple Developer 帳號的價值，需要將 `jki-agent` 打包為標準 `.app` 格式，並實作符合 Apple 安全規範的程式碼簽署 (Codesigning) 與公證 (Notarization) 流程。

## 2. 核心任務 (Tasks)
- [x] **App 結構化**:
    - [x] 建立 `jki-agent.app` 物理目錄結構。
    - [x] 撰寫 `Info.plist` (設定 `LSUIElement=true` 與 Bundle ID)。
    - [x] 轉換 `icon.png` 為 macOS 原生 `icon.icns` 格式。
- [x] **權限配置 (Entitlements)**:
    - [x] 撰寫 `entitlements.plist`，開啟 Hardened Runtime 並授權 Keychain 存取。
- [x] **自動化建構優化**:
    - [x] 在 `Makefile` 中新增 `make bundle` 指令。
    - [x] 實作 `scripts/sign_macos.sh` 腳本，封裝 `codesign` 流程。
- [x] **公證流程整合**:
    - [x] 撰寫 `scripts/notarize_macos.sh`，對接 Apple `notarytool`。
- [x] **結案報告 (Mandatory)**:
    - [x] **必須執行 `write_file` 產出 `missions/mission-macos-packaging-report.md`。**

## 3. 涉及檔案 (Files Involved)
- `Makefile`
- `crates/jki-agent/Info.plist` (New)
- `crates/jki-agent/jki.entitlements` (New)
- `scripts/sign_macos.sh` (New)
- `scripts/notarize_macos.sh` (New)

---
*Status: Delegated by Architect. Professional Distribution Focus.*
