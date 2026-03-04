# Mission: jki 智慧序號解析與 Double Dash 保護 (Smart Index & Double Dash)

## 1. 背景 (Context)
目前的 `jki` 參數解析對單獨數字的意圖識別不足，導致 `jki 1 -` 這種極速指令失效。需要根據 `docs/jki-cli-spec.md` 附錄 C 的新規範進行重構。

## 2. 核心任務 (Tasks)
- [ ] **解析邏輯重構 (`crates/jki/src/main.rs`)**:
    - [ ] 辨識 `--` (Double Dash) 保護態。
    - [ ] 實作智慧序號提取：移除 `len() > 1` 的舊限制，支援單一數字參數提取為 `index_candidate`。
- [ ] **決策矩陣實作**:
    - [ ] **衝突預檢**：在選中 Index 前，執行 Pattern 搜尋以偵測潛在衝突。
    - [ ] **防禦性提示**：若存在衝突（如 `FF14`），在 stderr 噴出 `Note: Use 'jki -- <IDX>' to search instead.`。
    - [ ] **優雅降級**：若 Index 超出範圍，自動將該數字併入 Pattern 重新進行搜尋。
- [ ] **物理驗證**:
    - [ ] 執行 `make release`。
    - [ ] 驗證 `jki 1 -` 能選中第 1 項。
    - [ ] 驗證 `jki -- 1` 執行搜尋而非選中第 1 項。
    - [ ] 驗證 `jki 14` 在有 `FF14` 時能選中第 14 項並給予 Note 提示。

## 3. 涉及檔案 (Files Involved)
- `crates/jki/src/main.rs`
- `docs/jki-cli-spec.md` (Reference Appendix C)
- `missions/mission-jki-smart-index-report.md` (New)

## 4. 驗收標準 (Exit Criteria)
- [ ] 產出 `missions/mission-jki-smart-index-report.md`。
- [ ] `jki 1 -` 必須能成功產出第 1 項的 OTP。
- [ ] 所有變更必須通過編譯。

---
*Status: Defined by Architect. Focus on Positional Semantics.*
