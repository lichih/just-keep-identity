# Mission: CLI Intelligence & Cleanup Tools

## 1. 背景與目標 (Background & Objective)
`jki` 是一套追求極致速度與人體工學的 CLI 系統。目前系統缺乏高效的去重與標籤清理工具，且子指令與資料搜尋的模糊匹配仍有提升空間。

本任務目標是：
- **引入 `fuzzy-matcher`**：取代手寫邏輯，實現具備權重排序（Scoring）的模糊匹配。
- **智慧解析**：實作 Fuzzy Subcommand Resolution，支援 `jkim iw` -> `import-winauth`。
- **事後去重**：實作 `jkim dedupe` 指令，提供「序號標記與物理清理 (-d/-k)」工具。
- **高亮顯示**：在搜尋結果中高亮匹配字元，提升透明度與信任感。

## 2. 策略與階段 (Strategy & Phases)

### Phase 1: Fuzzy 智慧升級 (Intelligence Upgrade)
1. **引入 `fuzzy-matcher` (Skim 演算法)**：整合至 `jki-core`。
2. **Fuzzy Subcommand**：在進入 `clap` 之前攔截參數，對子指令執行模糊解析。
   - 優點：不再需要手動維護 `alias`。
   - 邏輯：根據分數（Score）自動映射或列出 `Did you mean...`。
3. **Fuzzy Account Search (排序與高亮)**：
   - **權重排序**：`Score` 最高者置頂。`jki ggl` 會精準命中 `Google` (Score: 100+) 而非 `Glow-Girl` (Score: 20)。
   - **視覺高亮範例**：
     ```text
     搜尋關鍵字: 'iw'
     結果展示:
     1) [Google] [i]mport-[w]inauth@gmail.com (ID: uuid-x)
     2) [Work] [i]nner-[w]eb (ID: uuid-y)
     (括號 [] 代表 ANSI 高亮/加底線)
     ```
4. **智慧解析邏輯**：

### Phase 2: 事後去重工具 (`jkim dedupe`)
1. **診斷與列舉 (Diagnosis)**：按 Secret 內容分組，並分配**全域唯一序號**。
   - `Group A` (Secret X): 1, 2, 3...
   - `Group B` (Secret Y): 4, 5, 6...
2. **標記與清除 (Mark-and-Sweep)**：
   - **`-d <idx>` (Discard)**：標記刪除物理 ID。
   - **`-k <idx>` (Keep)**：組內排除法，標記保留此項並刪除該組其餘所有影子條目。
3. **內部實作紀律**：
   - 物理操作（Sweep）必須基於 ID。
   - **二選一原子性**：同步修改 YAML 與 `.age` 加密金庫。
   - **衝突診斷**：針對同序號同時標記 -k/-d 或同組多個 -k 時報錯。

### Phase 3: 知識資產與手冊硬化 (Knowledge Asset SSoT)
1. **建立中控資產庫 (Assets-Based Architecture)**：
   - 在 `crates/jkim/assets/` 建立模組化 Markdown 資源。
   - 採用 **高效嵌入策略**：對長篇 guide 使用 `include_bytes!` 以減少 binary size 壓力。
2. **單一來源引用與渲染 (Rendering SSoT)**：
   - **終端渲染**：引入 `termimad` 實作 Markdown 到 ANSI 的轉換。
   - **手冊合成**：利用 `anstyle_roff` 將 ANSI 樣式字串精準映射為 ROFF 宏，確保 `jkim man` 與終端輸出具備完全一致的語義視覺。
3. **動態引導與發現**：
   - `jkim completions` 與 `jkim status` 均引用 assets 進行提示。
   - 實作 `jkim man` 子指令，透過 `xtask` 預生成的 `.1` 檔案提供離線權威手冊。

## 3. 指令範例 (Command Examples)

### 3.1. `jkim iw` (Fuzzy Subcommand)
自動識別為 `import-winauth`：
```text
[Fuzzy] Running 'import-winauth' (iw matched)
```

### 3.2. `jkim dedupe -k2 -d6` (Cleanup)
- Group A: 僅保留 2，刪除 1, 3。
- Group B: 刪除 6，保留 4, 5, 7。

### 3.3. `jkim dedupe` 安全確認畫面範例
```text
!!! WARNING: PERMANENT DELETION !!!
The following entries will be removed from Metadata and Secrets vault:
  - [Google] user1@gmail.com (ID: uuid-a)
  - [None] Google user1 (ID: uuid-c)
  - [None] GitHub (ID: uuid-g)

Total to delete: 3 entries.
Proceed with deletion? [y/N]: _
```

## 4. 完成定義 (Definition of Done)
- [ ] 成功引入 `fuzzy-matcher` 並在指令解析中生效。
- [ ] `jki` 搜尋結果具備權重排序與高亮。
- [ ] `jkim dedupe` 指令完成並通過「組內排除與精準刪除」測試。
- [ ] **安全防護**：`dedupe` 物理刪除前必須列出明細並提供二次確認。
- [ ] 規格文件 `docs/jki-cli-spec.md` 已同步更新。
