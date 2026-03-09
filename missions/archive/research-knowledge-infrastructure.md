# Research Mission: Knowledge Management Infrastructure (Deep Research Request)

> **IMPORTANT: Report Delivery Specifications**
> - **Filename Suggestion**: `research-knowledge-infrastructure-report.md`
> - **Primary Audience**: Senior Rust Engineer (The Agent)
> - **Format**: Markdown (.md) with clear technical headers and actionable code blocks.

## 1. 任務背景 (Task Context)
正在開發名為 `jki` 的 Rust CLI 2FA 管理器。目前核心挑戰在於：**建立一套「知識資產單一真理 (Knowledge Asset SSoT)」系統**。
- **資料來源**：模組化的 Markdown 檔案 (assets/*.md)，包含引導、衝突救援、同步紀律等。
- **輸出渠道**：
    1. **Terminal**：執行時動態渲染 Markdown (帶 ANSI 樣式)。
    2. **Man Page**：發行時將 Markdown 轉化為 ROFF 並注入 `clap_mangen`。
- **目前預研方案**：`cargo xtask` (開發輔助)、`mandown` (ROFF 轉換)、`termimad` (終端渲染)、`include_str!` (程式碼嵌入)。

## 2. 深度調研需求 (Research Requests)

### Q1: 業界標準與優秀案例 (Benchmarking)
- 調研頂尖 Rust CLI (如 `bat`, `ripgrep`, `cargo`, `fd`, `eza`, `delta`) 如何管理其長篇引導文字與外部文件？
- 它們是否採用了比 `include_str!` 更自動化、更具備「類型安全」的資產管理模式？

### Q2: Man Page 生成管線 (Man Page Pipeline)
- 評估 `clap_mangen` 與 `mandown` 整合的可行性與已知問題（例如：列表縮進、粗體轉換失效）。
- 是否有比 `mandown` 更強大、更符合 Rust 慣例的 Markdown-to-ROFF 轉換方案？(例如：`pandoc` 之外的原生 Rust 解決方案)。

### Q3: 編譯期資產完整性校驗 (Integrity Validation)
- 如何在 `build.rs` 或 `xtask` 中實作一個「寧濫勿殺」的 Regex 掃描器，確保程式碼中所有的資產引用點都有對應實體檔案？
- 是否有現成的 crate 可以在「編譯期」而非運行期，強制驗證 `include_str!` 指向的內容與其標籤的一致性？

### Q4: 多平台相容性與效能 (Platform & Perf)
- 評估大量嵌入 Markdown 檔案對 Binary 啟動延遲與體積的具體影響。
- 列出在 macOS/Linux (troff/groff) 生成 Man Page 時最常見的排版相容性地雷。

## 3. 預期報告內容 (Expected Report Sections)
1. **方案評選表**：比較 `xtask` 腳本、`build.rs` 模式與 `cargo-make` 等第三方工具的優劣。
2. **實作範本 (Rust)**：提供一個將 Markdown 內容動態注入 `clap_mangen` 的具體 `xtask` 程式碼範例。
3. **工具鏈推薦**：一份精選的 Crates 清單 (如 `fuzzy-matcher`, `termimad`, `mandown` 等的最新評測)。

---
*Status: Ready for External Execution via Gemini Web App Deep Research.*
