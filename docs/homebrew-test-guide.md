# Homebrew 純淨隔離測試指南 (Zero-Trust Method)

本指南提供一種「完全不依賴系統現有 Homebrew」的測試方法。我們將在臨時目錄中下載一個全新的 Homebrew 實體，從零開始執行安裝測試。

## 1. 建立完全隔離的測試空間

我們將在專案根目錄下建立一個 `brew-test` 資料夾（已在 `.gitignore` 中排除）。

```bash
# 1. 進入專案根目錄並建立測試目錄
cd /Users/lichih/code/just-keep-identity
export JKI_PURITY_DIR="$(pwd)/brew-test"
rm -rf "$JKI_PURITY_DIR" # 確保絕對乾淨
mkdir -p "$JKI_PURITY_DIR"

# 2. 下載全新的 Homebrew 核心
curl -L https://github.com/Homebrew/brew/tarball/master | tar xz --strip 1 -C "$JKI_PURITY_DIR"
```

## 2. 配置臨時環境變數與 Tap

我們必須讓臨時的 brew 知道它的家在哪裡，並連結正式的 Tap。

```bash
# 1. 封閉環境設定
export PATH="$JKI_PURITY_DIR/bin:/usr/bin:/bin:/usr/sbin:/sbin"
export HOMEBREW_PREFIX="$JKI_PURITY_DIR"
export HOMEBREW_CELLAR="$JKI_PURITY_DIR/Cellar"
export HOMEBREW_REPOSITORY="$JKI_PURITY_DIR"
export HOMEBREW_CACHE="$JKI_PURITY_DIR/cache"

# 2. 建立本地 Tap 目錄並連結 (Homebrew 4.0+ 必須有 Tap 結構)
# 這樣可以直接測試你本地還沒 push 的 jki.rb
export TAP_DEST="$JKI_PURITY_DIR/Library/Taps/lichih/homebrew-jki/Formula"
mkdir -p "$TAP_DEST"
cp /Users/lichih/code/just-keep-identity/docs/homebrew-jki.rb "$TAP_DEST/jki.rb"

# 3. 驗證目前使用的 brew 位置
which brew
brew --version
```

## 3. 執行安裝測試

### A. 測試正式 Tap 安裝 (從 GitHub)
如果你想測試已經 Push 到 GitHub 的版本：
```bash
brew tap lichih/jki
brew install jki
```

### B. 測試本地修改 (Pre-push 驗證)
如果你已經執行了上面的 `cp` 步驟，直接執行安裝即可：
```bash
# 這裡會優先使用你剛才複製到 Library/Taps/... 的本地 jki.rb
brew install --verbose --debug jki

# 驗證二進位檔路徑
"$JKI_PURITY_DIR/bin/jki" --version
```

### C. 測試源碼編譯 (Source Build)
```bash
brew uninstall jki
brew install --build-from-source --verbose --debug jki
```

## 4. 為什麼這個方案更可信？

1.  **無污染**：它沒有使用你系統中 `/opt/homebrew` 的任何檔案。
2.  **可複製性**：這就是 Homebrew 在 CI (GitHub Actions) 上的運作方式。
3.  **物理隔離**：所有的二進位檔、快取、Metadata 全都在 `$JKI_PURITY_DIR` 下。

## 5. 清理

測試結束後，直接關閉終端機或 `unset` 變數，並刪除目錄即可：
```bash
rm -rf "$JKI_PURITY_DIR"
```
