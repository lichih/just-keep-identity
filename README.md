# Just Keep Identity (jki)

**JK Suite 極速 MFA 數位金庫** - 專為開發者設計的身分授權管理方案。

## 核心特性
- **極速 (Speed)**：`jki` 查詢器 Cold Start < 3ms，Hot Path 直接輸出 OTP。
- **人體工學 (Ergonomics)**：右手單手操作 (Micro-Roll)，`j-k-i` 指令集。
- **安全 (Security)**：原生支持 Windows Hello ESS / macOS Touch ID，整合背景 Agent (jki-agent)。
- **Unix 哲學**：標準 Exit Code、`--` 參數支援與 `stdout (-)` 輸出整合。

## 專案結構 (Rust Workspace)
- `crates/jki`：極速查詢器 (CLI Executor)。
- `crates/jkim`：管理中心 (TUI & CRUD)。
- `crates/jki-agent`：背景授權服務 (Session Agent)。
- `crates/jki-core`：共享核心邏輯 (TOTP, Crypto)。

## 安裝與快速開始

### 快速安裝
您可以透過專案提供的安裝腳本快速完成編譯與路徑設置：
```bash
# 下載專案後執行
./install.sh
```
或是使用 `Makefile`：
```bash
make install
```

### 開發者指令
專案提供 `Makefile` 簡化開發流程：
- `make dev`：編譯 Debug 版本或啟動 `cargo watch`。
- `make test-all`：執行全工作區測試。
- `make release`：編譯 Release 版本。
- `make clean`：清理編譯產物。

### 基本使用
1. **初始化與管理**：使用 `jkim` 匯入或建立帳號。
2. **極速查詢**：
   ```bash
   jki [pattern]
   ```

## 授權
MIT OR Apache-2.0
