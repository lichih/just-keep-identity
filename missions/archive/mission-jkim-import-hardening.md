# Mission: jkim 匯入邏輯硬化 (Import Logic Hardening)

## 1. 背景 (Context)
目前的 `jkim import-winauth` 實作存在「自作聰明」的混合狀態轉換邏輯，且頻繁跳出 y/n 提示，違反了 JKI 的極速哲學與安全職責隔離。

**目標**：依照 `docs/jki-prd-spec.md` 附錄 A 的新規範，重構 `handle_import_winauth` 函式。

## 2. 核心任務 (Tasks)
- [ ] **偵測邏輯重構**:
    - [ ] 物理偵測：辨識 `.age`, `.json` 與 `metadata` 的存在狀態。
    - [ ] 損壞檢查：若 Meta 存在但 Secrets 全失，直接報錯停止。
- [ ] **認證邏輯調整**:
    - [ ] 呼叫 `acquire_master_key` 時傳入 `None` 作為 SecretStore。
    - [ ] 實作 Fail-fast：若現狀為加密態且取得 Key 失敗，直接停止。
- [ ] **儲存決策矩陣實作**:
    - [ ] **維持加密態**：已有 `.age` 且認證成功 -> 直接更新，**0 詢問**。
    - [ ] **維持明文態**：已有 `.json` 且認證失敗 -> 直接更新，**0 詢問**。
    - [ ] **升級機會**：已有 `.json` 且認證成功 -> 提示升級 `[y/N]`。
    - [ ] **初始狀態**：
        - [ ] 有 Key -> 建立 `.age`，**0 詢問**。
        - [ ] 無 Key -> 提示建立明文 `[y/n]`。
- [ ] **代碼清理**: 
    - [ ] 移除舊有的 `if has_master_key { ... }` 混合判斷代碼。
    - [ ] 修正 `jkim/src/main.rs` 中的變數未使用警告。
- [ ] **物理驗證**: 執行 `make release` 確保編譯通過。

## 3. 涉及檔案 (Files Involved)
- `crates/jkim/src/main.rs`
- `docs/jki-prd-spec.md` (Reference for Appendix A)
- `missions/mission-jkim-import-hardening-report.md` (New)

## 4. 驗收標準 (Exit Criteria)
- [ ] 產出 `missions/mission-jkim-import-hardening-report.md`。
- [ ] `jkim import` 在「原本加密」或「無金鑰明文」時不應跳出任何 y/n。
- [ ] 所有變更必須通過編譯。

---
*Status: Defined by Architect. Focus on Appendix A compliance.*
