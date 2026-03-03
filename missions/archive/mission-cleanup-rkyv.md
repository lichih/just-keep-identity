# Mission: Cleanup rkyv & Binary Optimization

## 1. Objective
移除專案中所有過時的 `rkyv` 二進位優化規劃與程式碼，落實「架構減法」。

## 2. Tasks
- [x] **Docs Cleanup**:
    - [x] 從 `docs/jki-prd-spec.md` 中移除 Phase 5 關於二進位優化 (rkyv) 的描述。
    - [x] 將 Phase 5 重新定義為 "Productization & Reliability"。
    - [x] 檢查 `README.md` 並移除 `rkyv` 的描述。
- [x] **Codebase Cleanup**:
    - [x] 從 `crates/jki-core/Cargo.toml` 中移除 `rkyv` 依賴。
    - [x] 從 `crates/jki-core/src/lib.rs` 中移除 `rkyv` 相關的匯入與屬性標記（如 `#[derive(Archive, ...)]`）。
- [x] **Verification**:
    - [x] 執行 `grep -r "rkyv" .` 確保沒有殘留（排除 `missions/` 目錄）。
    - [x] 執行 `cargo check` 確保編譯正常。

## 3. Deliverables
- [x] 清理後的檔案。
- [x] 驗證報告 `missions/mission-cleanup-rkyv-report.md`。
