# Mission Report: macOS 官方打包與公證預備 (Packaging & Signing)

## 1. 執行摘要 (Executive Summary)
已成功為 `jki-agent` 建立 macOS 標準 `.app` 打包流程。包括實作 `Info.plist`、權限配置 `jki.entitlements`、自動化圖示轉換，以及 Codesigning 與 Notarization 腳本整合至 `Makefile`。

## 2. 完成任務 (Completed Tasks)
- [x] **App 結構化**:
    - [x] 建立 `crates/jki-agent/Info.plist` (設定 `LSUIElement=true`, Bundle ID: `com.just-keep.identity.agent`)。
    - [x] 在 `Makefile` 中實作 `bundle-icon` 流程，利用 `sips` 與 `iconutil` 將 `icon.png` 轉換為 `icon.icns`。
    - [x] 建立 `jki-agent.app` 的標準目錄結構 (`Contents/MacOS`, `Contents/Resources`)。
- [x] **權限配置 (Entitlements)**:
    - [x] 撰寫 `crates/jki-agent/jki.entitlements`，開啟 Hardened Runtime 並授權 Keychain 存取權限。
- [x] **自動化建構優化**:
    - [x] 在 `Makefile` 中新增 `make bundle` 指令。
    - [x] 實作 `scripts/sign_macos.sh`，支援 `codesign` 流程與 Hardened Runtime。
- [x] **公證流程整合**:
    - [x] 撰寫 `scripts/notarize_macos.sh`，對接 Apple `notarytool` 並支援 `stapler`。

## 3. 產出檔案列表 (Artifacts)
- `Makefile` (更新: 新增 `bundle`, `sign`, `notarize` 目標)
- `crates/jki-agent/Info.plist` (新增)
- `crates/jki-agent/jki.entitlements` (新增)
- `scripts/sign_macos.sh` (新增)
- `scripts/notarize_macos.sh` (新增)

## 4. 後續建議 (Next Steps)
- **環境變數設定**: 使用者需在 CI/CD 或本地環境設定 `SIGNING_IDENTITY` (Developer ID Application), `APPLE_ID`, `TEAM_ID` 與 `AC_PASSWORD` (App-specific password)。
- **公測驗證**: 在實體 macOS 環境執行 `make bundle && make sign` 驗證簽署後的 App 是否能正確讀取 Keychain 且不被系統封鎖。

---
*Status: Completed. Ready for Distribution.*
