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
- `crates/jki-core`：共享核心邏輯 (TOTP, Crypto, rkyv)。

## 快速開始
1. **編譯**：
   ```bash
   cargo build --release
   ```
2. **初始化與管理**：使用 `jkim` 匯入或建立帳號。
3. **極速查詢**：
   ```bash
   jki [pattern]
   ```

## 授權
MIT OR Apache-2.0
