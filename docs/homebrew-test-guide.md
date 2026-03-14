# Homebrew 純淨隔離測試指南 (Zero-Trust Method)

本指南提供一種「完全不依賴系統現有 Homebrew」的測試方法，用以驗證 **外部使用者** 透過 GitHub Tap 安裝 `jki` 的真實體驗。

## 1. 建立完全隔離的測試空間

我們將在專案根目錄下建立一個 `brew-test` 資料夾（已在 `.gitignore` 中排除）。

```bash
# 1. 建立並進入測試目錄
cd /Users/lichih/code/just-keep-identity
export JKI_PURITY_DIR="$(pwd)/brew-test"
rm -rf "$JKI_PURITY_DIR"
mkdir -p "$JKI_PURITY_DIR"

# 2. 下載全新的 Homebrew 核心
curl -L https://github.com/Homebrew/brew/tarball/master | tar xz --strip 1 -C "$JKI_PURITY_DIR"
```

## 2. 配置環境變數

為了讓隔離版 `brew` 能正常工作，必須設定以下變數：

```bash
# 1. 指向測試目錄的 bin
export PATH="$JKI_PURITY_DIR/bin:/usr/bin:/bin:/usr/sbin:/sbin"

# 2. 定義 Homebrew 隔離路徑
export HOMEBREW_PREFIX="$JKI_PURITY_DIR"
export HOMEBREW_CELLAR="$JKI_PURITY_DIR/Cellar"
export HOMEBREW_REPOSITORY="$JKI_PURITY_DIR"
export HOMEBREW_CACHE="$JKI_PURITY_DIR/cache"

# 3. 關鍵：禁用自動更新
# 因為我們是透過 tarball 安裝的，brew 無法對自身執行 git update
# 若不禁用，執行 tap 時會觸發 update-report 錯誤
export HOMEBREW_NO_AUTO_UPDATE=1

# 4. 允許從 API 獲取索引（建議取消禁令以提升效能）
unset HOMEBREW_NO_INSTALL_FROM_API

# 驗證環境
which brew
brew --version
```

## 3. 執行使用者真實路徑測試

現在的環境與一般使用者完全一致，且直接從 GitHub 獲取資料。

### A. 執行 Tap 與安裝
```bash
# 1. 連結 GitHub 上的正式 Tap (這會執行 git clone)
brew tap lichih/jki

# 2. 執行安裝 (會下載 GitHub Release 的二進位檔)
brew install jki

# 3. 驗證執行
jki --version
jkim --version
```

### B. 測試源碼編譯 (模擬其他平台)
```bash
brew uninstall jki
brew install --build-from-source jki
```

## 4. 清理

```bash
rm -rf "$JKI_PURITY_DIR"
```
