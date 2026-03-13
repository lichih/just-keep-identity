# Mission: CLI Intelligence & Cleanup Tools

## 1. 背景與目標 (Background & Objective)
`jki` 是一套追求極致速度與人體工學的 CLI 系統。目前系統缺乏高效的去重與標籤清理工具，且子指令與資料搜尋的模糊匹配仍有提升空間。

本任務目標是：
- **引入 `fuzzy-matcher`**：取代手寫邏輯，實現具備權重排序（Scoring）的模糊匹配。
- **智慧解析**：實作 Fuzzy Subcommand Resolution，支援 `jkim iw` -> `import-winauth`。
- **事後去重**：實作 `jkim dedupe` 指令，提供「序號標記與物理清理 (-d/-k)」工具。
- **高亮顯示**：在搜尋結果中高亮匹配字元，提升透明度與信任感。

## 2. 策略與階段 (Strategy & Phases)

### Phase 1: Fuzzy 智慧升級 (Intelligence Upgrade) - [In Progress]
1. [x] **引入 `fuzzy-matcher` (Skim 演算法)**：整合至 `jki-core` 並實作 `MatchedAccount` 結構。
2. [x] **智慧計分矩陣**：實作 `adjust_score`，建立「Issuer > Name」與「Prefix Bonus」權重邏輯。
3. [x] **搜尋結果高亮**：實作 ANSI 渲染，視覺化解噪。
4. [x] **智慧自動選中 (Dominant Winner)**：實作「顯著差異 (Gap >= 40)」判定，實現極速產碼。
5. [ ] **提示與診斷硬化 (Feedback Transparency)**：
   - 在歧義 (Ambiguous) 時輸出分數差距對比。
   - 優化自動選中時的 stderr 提示，確保使用者具備感知。
   - (Optional) 實作 `--debug-score` 診斷計分邏輯。
6. [ ] **Fuzzy Subcommand**：在進入 `clap` 之前攔截參數，對子指令執行模糊解析 (如 `iw` -> `import-winauth`)。
   - 邏輯：根據分數自動映射或列出 `Did you mean...`。

### Phase 2: 事後去重工具 (`jkim dedupe`) - [Completed]
1. [x] **診斷與列舉 (Diagnosis)**：按 Secret 內容分組，並分配**全域唯一序號**。
2. [x] **標記與清除 (Mark-and-Sweep)**：
3. [x] **內部實作紀律**：

### Phase 3: 知識資產與手冊硬化 (Knowledge Asset SSoT) - [Completed]
1. [x] **建立中控資產庫 (Assets-Based Architecture)**：
   - 在 `crates/jkim/assets/` 建立模組化 Markdown 資源。
   - 採用 **高效嵌入策略**：對長篇 guide 使用 `include_str!` 以減少 runtime 讀取。
2. [x] **單一來源引用與渲染 (Rendering SSoT)**：
   - **終端渲染**：引入 `termimad` 實作 Markdown 到 ANSI 的轉換。
   - **手冊合成 (Deferred)**：利用 `anstyle_roff` 將 ANSI 樣式字串映射為 ROFF (先標記放置，目前依賴終端 `jkim man` 提供服務)。
3. [x] **動態引導與發現**：
   - `jkim completions` 與 `jkim status` 均引用 assets 進行提示。
   - 實作 `jkim man` 子指令，提供直覺的離線權威手冊。

## 3. 指令範例 (Command Examples)

### 3.1. `jkim iw` (Fuzzy Subcommand)
自動識別為 `import-winauth`：
```text
[Fuzzy] Running 'import-winauth' (iw matched)
```

### 3.2. `jkim dedupe -k2 -d6` (Cleanup)
偵測到重複金鑰，分組列出並分配全域序號：
```text
Group A: (Secret: JBSW...3PXP)
  1) [None] Google user1 (ID: uuid-a)
  2) [Google] user1@gmail.com (ID: uuid-b)
  3) [Old] google-backup (ID: uuid-c)

Group B: (Secret: XOXO...999)
  4) GitHub lichih (ID: uuid-d)
  5) [gitlab] lichihwu (ID: uuid-e)
  6) [None] git (ID: uuid-f)
  7) [Test] temp (ID: uuid-g)
```

執行 `jkim dedupe -k2 -d6` 的邏輯效果：
- **Group A**：指定保留 2，系統自動將同組的 1, 3 標記為刪除 (Mark-and-Sweep)。
- **Group B**：指定刪除 6，其餘 4, 5, 7 預設保留。

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
- [x] 成功引入 `fuzzy-matcher` 並在指令解析中生效。
- [x] `jki` 搜尋結果具備權重排序與高亮。
- [x] `jkim dedupe` 指令完成並通過「組內排除與精準刪除」測試。
- [x] **安全防護**：`dedupe` 物理刪除前必須列出明細並提供二次確認。
- [x] 規格文件 `docs/jki-cli-spec.md` 已同步更新。
