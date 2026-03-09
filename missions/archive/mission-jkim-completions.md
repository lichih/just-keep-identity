# Mission: jkim Shell 自動補全支援 (Shell Completions)

## 1. 背景 (Context)
為了提升 `jkim` 的易用性，需要實作 Shell 自動補全腳本生成功能。使用者應能透過 `jkim completions <SHELL>` 取得對應的補全程式碼。

## 2. 核心任務 (Tasks)
- [x] **依賴更新 (`crates/jkim/Cargo.toml`)**:
    - [x] 加入 `clap_complete = "4.4"`。
- [x] **子指令新增 (`crates/jkim/src/main.rs`)**:
    - [x] 在 `Commands` enum 中新增 `Completions { shell: Shell }`。
    - [x] `Shell` 應來自 `clap_complete::Shell`。
- [x] **邏輯實作**:
    - [x] 在 `main` 或 `handle_completions` 中，使用 `clap_complete::generate` 生成腳本並輸出至 `stdout`。
- [x] **物理驗證**:
    - [x] 執行 `make release` 確保編譯。
    - [x] 測試 `jkim completions bash` 是否輸出有效的 bash 腳本。
    - [x] 測試 `jkim completions zsh` 是否輸出有效的 zsh 腳本。

## 3. 涉及檔案 (Files Involved)
- `crates/jkim/Cargo.toml`
- `crates/jkim/src/main.rs`
- `missions/archive/mission-jkim-completions-report.md` (New)

## 4. 驗收標準 (Exit Criteria)
- [x] 產出 `missions/archive/mission-jkim-completions-report.md`。
- [x] `jkim completions bash` 成功輸出且不報錯。
- [x] 通過 `cargo check` 驗證。

---
*Status: Defined by Architect. Enhancing CLI UX.*
