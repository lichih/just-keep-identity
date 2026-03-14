# Homebrew 純淨隔離測試指南 (Zero-Trust Method)

本指南提供一種「完全不依賴系統現有 Homebrew」的測試方法，用以驗證 **外部使用者** 透過 GitHub Tap 安裝 `jki` 的真實體驗。

## 1. 建立完全隔離的測試環境 (模擬使用者安裝)

我們將在專案根目錄下建立一個 `brew-test` 資料夾。**為了確保 brew 行為與真實使用者一致，我們必須使用 Git 進行安裝。**

```bash
# 1. 建立並進入測試目錄
cd /Users/lichih/code/just-keep-identity
export JKI_PURITY_DIR="$(pwd)/brew-test"
rm -rf "$JKI_PURITY_DIR"
mkdir -p "$JKI_PURITY_DIR"

# 2. 以 Git Clone 方式安裝 Homebrew (這才是真實使用者的環境)
git clone --depth 1 https://github.com/Homebrew/brew "$JKI_PURITY_DIR"
```

## 2. 配置路徑

只需設定 `PATH` 與路徑變數，**不要設定任何禁用更新的變數**，以維持環境的原始性。

```bash
# 1. 指向測試目錄的 bin
export PATH="$JKI_PURITY_DIR/bin:/usr/bin:/bin:/usr/sbin:/sbin"

# 2. 定義 Homebrew 隔離路徑
export HOMEBREW_PREFIX="$JKI_PURITY_DIR"
export HOMEBREW_CELLAR="$JKI_PURITY_DIR/Cellar"
export HOMEBREW_REPOSITORY="$JKI_PURITY_DIR"
export HOMEBREW_CACHE="$JKI_PURITY_DIR/cache"

# 驗證環境 (這時 brew 應該是正常的 Git 倉庫)
which brew
brew --version
```

## 3. 執行使用者真實路徑測試

現在的環境與一般使用者完全一致。

### A. 執行 Tap 與安裝
```bash
# 1. 連結 GitHub 上的正式 Tap
brew tap lichih/jki

# 2. 執行安裝
brew install jki

# 3. 驗證執行
jki --version
jkim --version
```

### B. 測試源碼編譯 (模擬 Linux/非 ARM 平台)
```bash
brew uninstall jki
brew install --build-from-source jki
```

## 4. 清理

```bash
rm -rf "$JKI_PURITY_DIR"
```
