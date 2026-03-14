# Homebrew 發佈指南 (Homebrew Distribution Guide)

本文件記載了如何將 JKI 透過 Homebrew Tap 進行發佈與更新的標準流程。

---

## 1. 架構說明

JKI 採用 **Hybrid Release** 策略：
- **macOS (ARM64)**: 提供預編譯 (Pre-built) 二進位檔以加速安裝。
- **其他平台**: 提供原始碼編譯 (Source Build)，需具備 Rust 環境。

---

## 2. 自動化發佈流程 (The Makefile Way)

我們已將發佈流程整合進 `Makefile`。

### 2.1 準備二進位包
這會編譯 Release 版本、封裝二進位檔，並計算 SHA256。
```bash
make brew-package
```

### 2.2 上傳至 GitHub Release
這會將產出的 `jki-macos-arm64.tar.gz` 上傳到目前的 Git Tag 對應的 GitHub Release。
```bash
make brew-dist
```

---

## 3. 更新 Formula

完成上傳後，需手動更新 Homebrew Formula (`docs/homebrew-jki.rb`)：

1.  複製 `make brew-package` 輸出的 **SHA256 雜湊值**。
2.  更新 `docs/homebrew-jki.rb` 中的 `sha256` 欄位（對應 `OS.mac? && Hardware::CPU.arm?` 區塊）。
3.  更新 `version` 欄位。

---

## 4. 更新 Tap 倉庫 (lichih/homebrew-jki)

JKI 的官方 Tap 位於 [https://github.com/lichih/homebrew-jki](https://github.com/lichih/homebrew-jki)。

更新步驟：
1.  複製 `docs/homebrew-jki.rb` 的內容。
2.  覆蓋 `homebrew-jki` 倉庫中的 `Formula/jki.rb`。
3.  提交並推送變更。

---

## 5. 使用者安裝指令

使用者只需執行以下指令即可安裝：
```bash
brew tap lichih/jki
brew install jki
```
