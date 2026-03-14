# Mission Report: 從「計數校驗」進化至「物理座標鎖定 (CAE)」的編輯安全革命

## 1. 任務背景與演進

- **階段一 (行數校驗)**：為了解決 Agent 對 `edit` 範圍的幻覺，實作了 `expectedOldLineCount` 必填機制。
- **階段二 (物理定址)**：發現 LLM 雖然能數行數，但對不可見字元與結尾換行的感知依然薄弱。為了達到最高穩定性，我們決定全面捨棄「模式搜尋」，改採「座標定址」。

## 2. 核心技術實作

在此版本中，`edit` 工具已被重構為 **Coordinate-Anchored Edit (CAE)** 引擎：

1.  **定址化參數**：`startLine` 與 `endLine` 成為核心必填項。
2.  **物理切片比對**：工具不再搜尋全檔案，而是直接「切出」座標區間的字串，執行 **Bit-by-bit** 嚴格比對。
3.  **零容忍縮排**：針對 Python/YAML 等語言，任何微小的縮排差異都會被攔截，有效杜絕邏輯錯位。

## 3. 實戰測試證據 (Test Evidence)

### 情境 A：座標完全匹配 (成功案例)

- **輸入**：`startLine: 2, endLine: 2`, `oldString` 與第 2 行完全一致。
- **結果**：編輯順利套用。
- **意義**：確信 Agent 正在正確的物理位址上工作。

### 情境 B：行號偏移攔截 (安全性案例)

- **行為**：Agent 試圖修改第 1 行內容，但座標誤填為 `lines 2-2`。
- **工具反應**：
  ```text
  Error: Atomic Edit Integrity Check Failed: The content at lines 2-2 does not match your oldString exactly.
  Actual content at lines 2-2: '''Line 2: Beta - UPDATED'''
  ```
- **效益**：物理性防止「改錯行」或「幻覺抹除」。

### 情境 C：縮排幻覺診斷 (開發輔助案例)

- **行為**：Agent 提供的 `oldString` 前方多了兩個空格，座標正確。
- **工具反應**：
  ```text
  Error: Atomic Edit Integrity Check Failed...
  Hint: The text matches if trimmed. Please check for leading/trailing whitespace or indentation differences.
  ```
- **效益**：立即修正 Agent 對專案縮排規範的認知，無需反覆重讀。

### 情境 D：敏感標籤保護 (Tier 0 案例)

- **行為**：Agent 試圖移除包含 `# Sensitive` 註解的整行。
- **工具反應**：`Error: Safety Guard: Your edit would remove a sensitive pattern...`
- **效益**：建立物理層級的「不准動」邊界。

## 4. 結語

現在的 `edit` 工具已成為一個專業的「合約執行器」。Agent 不再被允許「猜測」檔案內容，必須與磁碟物理狀態達成 100% 的同步。這將顯著提升 `gemini-3-flash` 在處理高敏感、大規模設定檔時的可靠性。

---

**紀錄者**：Antigravity (Opencode Agent)
**歸檔日期**：2026-03-14
