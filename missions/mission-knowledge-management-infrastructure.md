# Mission: Knowledge Management Infrastructure (Asset-Code Sync)

## 1. 背景與目標 (Background & Objective)
隨著 `jki` 系統的人體工學強化，大量的「指示性內容」（如同步紀律、衝突救援、去重原理）被實體化為獨立的 Markdown 資產。手動管理這些資產與 Rust 程式碼（`include_str!`）以及 Man Page 生成邏輯之間的關聯極其困難且容易出錯。

本任務目標是：
- **建立雙軌制管理**：結合 **腳本自動化（確保物理連結正確）** 與 **LLM（負責高品質內容）**。
- **實作資產映射工具**：開發一套掃描腳本，精準找出系統中所有「資產需求點」與「資產供應點」。
- **寧濫勿殺原則**：腳本工具應盡可能列出所有潛在關聯，再由 LLM 執行精確的代碼插入與內容同步。

## 2. 策略與階段 (Strategy & Phases)

### Phase 1: 標籤規範與資產結構 (Standardization)
1. **定義標籤語法**：
   - 程式碼端使用註解標記需求點：`// @JKI_ASSET(guide_sync)`。
   - 建立中控資產目錄：`crates/jkim/assets/*.md`。
2. **建立資產清單 (Registry)**：
   - 一個簡易的索引檔（如 `assets.yaml`），描述每個資產的 ID、用途與預期嵌入位置（CLI tip, Man Page SECTION 等）。

### Phase 2: 資產管理與編譯期校驗 (Infra & Linting)
1. **實作 `xtask` 基礎設施**：
   - 在專案 root 建立 `xtask` crate。
   - 配置 `.cargo/config.toml` 別名：`xtask = "run --package xtask --"`。
2. **編譯期守門員 (`build.rs`) - 寧濫勿殺校驗**：
   - 引入 `regex` 與 `walkdir` 於 `build-dependencies`。
   - **掃描邏輯**：遍歷 `src/**/*.rs` 尋找 `include_str!`、`include_bytes!` 或 `@JKI_ASSET` 註解。
   - **強制中斷**：若引用的資產檔案不存在，發出 `cargo:warning` 並執行 `panic!` 中斷編譯，確保發行版完整性。
3. **高效嵌入策略**：
   - **大小權衡**：針對 > 10KB 的 Markdown 檔案，優先使用 `include_bytes!` 嵌入以減少編譯器元數據壓力，運行時再轉換為 `&str`。

### Phase 3: 知識資產渲染與手冊合成 (Rendering & SSoT)
1. **終端渲染 (Rich UI)**：
   - 引入 `termimad` crate。將 Markdown 轉化為帶 ANSI 色碼的字串。
   - 實作「匹配高亮」提示，增加搜尋透明度。
2. **手冊合成管線 (`cargo xtask man`)**：
   - 整合 `clap_mangen` 與 **`anstyle_roff`**。
   - **樣式橋接**：利用 `anstyle_roff` 將 `termimad` 產出的 ANSI 樣式字串轉換為 ROFF 宏（.B, .I），確保終端提示與手冊頁面共用同一套樣式定義。
3. **多平台相容性 (macOS/Linux)**：
   - 避免使用 GNU 特有的 ROFF 擴展（如 .SY），以確保在 macOS (mandoc) 上的排版正確性。

## 3. 預期效果 (Expected Outcome)
- **純 Rust 工具鏈**：無需 Python 環境，全開發週期皆由 `cargo` 驅動。
- **極致 SSoT**：一份 Markdown 同時供 CLI Tip、Man Page 使用。

## 4. 完成定義 (Definition of Done)
- [ ] `scripts/asset_linter.py` 實作完成，可精準掃描 `@JKI_ASSET` 標籤。
- [ ] 所有目前的指示性文字（Sync, Rescue, Dedupe）已成功遷移至 `assets/`。
- [ ] 腳本報告能正確引導 LLM 完成代碼端與手冊端的同步更新。
- [ ] 整合進入專案檢核流程，確保 0 孤立標籤。
