# macOS 軟體發佈全指南：從原始碼到官方公證 (Notarization)

這份文件記載了如何將 macOS 應用程式（特別是 Rust 編譯的二進位檔）轉化為具備官方簽署與公證、且能通過 Gatekeeper 檢核的正式產品。

---

## 1. 身份認證：憑證申請與導入 (CLI 模式)

當系統 UI 工具 (Keychain Access) 不穩定時，使用 CLI 是最可靠的路徑。

### 1.1 產生憑證簽署要求 (CSR)
```bash
# 產生私鑰與 CSR (不進 Git)
openssl req -nodes -newkey rsa:2048 
    -keyout private.key 
    -out request.csr 
    -subj "/emailAddress=your@email.com/CN=Your Name/C=TW"
```

### 1.2 轉換與導入 Keychain
下載 Apple 產出的 `.cer` 檔案後，需執行以下轉換以讓系統識別「憑證 + 私鑰」的配對身份：

```bash
# 結合為 P12 (關鍵：使用 -legacy 以相容 macOS security 指令)
openssl pkcs12 -export -legacy 
    -out identity.p12 
    -inkey private.key 
    -in distribution.cer 
    -name "Distribution Identity" 
    -passout pass:1234

# 導入系統鑰匙圈
security import identity.p12 -k ~/Library/Keychains/login.keychain-db -P 1234
```

---

## 2. 封裝與權限配置 (Packaging)

### 2.1 標準 `.app` 結構
一個合規的 Bundle 必須包含：
*   `Contents/MacOS/`：主要執行檔。
*   `Contents/Resources/`：`icon.icns` 圖標檔。
*   `Contents/Info.plist`：元數據配置。

### 2.2 Info.plist 關鍵欄位
*   `LSUIElement`: 設為 `true` 則應用程式作為背景 Agent 執行（無 Dock 圖示）。
*   `CFBundleIdentifier`: 唯一的 Bundle ID。

### 2.3 Entitlements (權限宣告)
為了通過 **Hardened Runtime**，必須明確宣告權限（例如 `jki.entitlements`）：
```xml
<key>com.apple.security.cs.allow-jit</key><true/>
<key>com.apple.security.personal-information.addressbook</key><true/> <!-- 根據需求開啟 -->
```

---

## 3. 工業級簽署 (Codesigning)

### 3.1 基礎簽署
```bash
codesign --force --options runtime 
         --entitlements app.entitlements 
         --sign "Developer ID Application: Your Name (TEAMID)" 
         "YourApp.app"
```

### 3.2 進階：ACL 權限共享 (避免重複彈窗)
若有多個組件（如 CLI 與 Agent）需存取同一 Keychain 項目，在寫入金鑰時應預先授權：
```bash
security add-generic-password -a account -s service -w secret 
         -T /path/to/jkim 
         -T /path/to/jki-agent
```

---

## 4. 官方公證流程 (Notarization)

公證是讓 App 脫離「來源不明」標籤的唯一方法。

### 4.1 上傳與掃描
```bash
# 需先封裝為 ZIP
zip -r YourApp.zip YourApp.app

# 使用 notarytool 提交 (需 App-specific Password)
xcrun notarytool submit YourApp.zip 
      --apple-id "your@email.com" 
      --team-id "TEAMID" 
      --password "xxxx-xxxx-xxxx-xxxx" 
      --wait
```

### 4.2 釘上公證票券 (Stapling)
```bash
# 使 App 在離線狀態下也能通過驗證
xcrun stapler staple YourApp.app
```

---

## 5. 驗證指令 (Final Checks)

發佈前必須通過這兩項物理檢查：

1.  **簽章完整性**：
    `codesign --verify --verbose --deep YourApp.app`
2.  **Gatekeeper 認可度**：
    `spctl --assess --verbose YourApp.app`
    *成功標誌：`accepted, source=Notarized Developer ID`*

---

## 6. 安全性實踐 (Architect's Note)

1.  **環境變數隔離**：將 `AC_PASSWORD` 存放在 `.env.macos` 並加入 `.gitignore`。
2.  **防止指令回顯**：在 Makefile 中使用 `@` 符號隱藏包含密碼的指令執行過程。
3.  **自動化腳本**：將公證邏輯封裝在腳本中，由腳本內部執行 `source .env`，避免機敏資料出現在系統日誌或 AI 傳輸參數中。
