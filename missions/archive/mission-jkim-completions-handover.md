# Handover: jkim Shell Completions & Documentation System

## 1. 目前現狀 (Current Status)
- `jkim completions <SHELL>` 已實作，但對一般使用者不友善（缺乏自動偵測、缺乏安裝指引）。
- 已討論出「文件與代碼分離」的重構方案，但因上下文讀取機制優化中而暫停。

## 2. 目標方案 (The Optimal Design)
為了符合工程直覺、確保安全且具備配套，計畫實作：
- **引導資源分離**：將安裝指令與解釋文字移出 `lib.rs`，存入 `crates/jkim/assets/completions_guide.md`，使用 `include_str!` 嵌入。
- **動態引導邏輯**：
    - `jkim completions` (無參數)：從 `$SHELL` 自動偵測環境，並在 `stderr` 顯示針對性的一鍵安裝指令與全 Shell 對照表。
    - `jkim completions <SHELL>` (有參數)：在 `stdout` 產出純淨腳本供管道操作。
- **權威手冊 (Man Pages)**：
    - 引入 `clap_mangen`。
    - 新增 `jkim man` 子指令，輸出 `.1` 格式的 manual page。
- **功能配套 (Status Tip)**：
    - 在 `jkim status` 結尾加入一行 `Tip`，引導使用者發現 `completions` 指令。

## 3. 待辦事項 (Pending Tasks)
- [ ] 恢復 `crates/jkim/Cargo.toml` 對 `clap_mangen` 的依賴。
- [ ] 重新建立 `crates/jkim/assets/completions_guide.md`。
- [ ] 以「全讀 (Full Read)」方式重寫 `crates/jkim/src/lib.rs` 中的 `handle_completions` 與 `handle_status`。

---
*Status: Paused. Waiting for context management improvements.*
