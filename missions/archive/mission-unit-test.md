# Mission: Unit Test Coverage Analysis

## 1. Objective
強化 JKI 的測試基礎，安裝覆蓋率工具並產生詳細的 Code Coverage 報告，確保 Phase 5 實作的核心功能均有測試覆蓋。

## 2. Tasks
- [x] **Setup Tooling**:
    - [x] 嘗試安裝 `cargo-tarpaulin` (`cargo install cargo-tarpaulin`)。
    - [x] 在 `Makefile` 中新增 `cov` target，用於產生 HTML 或文字報告。
- [x] **Generate Report**:
    - [x] 執行 `cargo tarpaulin --workspace --out Html`。
    - [x] 分析目前的覆蓋率數據，特別是 `jki-core` 與 `jkim`。
- [x] **Improve Tests (If needed)**:
    - [x] 若核心邏輯（如 `force-age`, `export`, `conflict-resolve`）覆蓋率不足，補強測試案例。
- [x] **Closure**:
    - [x] 撰寫 `missions/mission-unit-test-report.md` 結案報告，列出各 crate 的覆蓋率數據。

## 3. Deliverables
- [x] `Makefile` 更新。
- [x] 覆蓋率報告 `tarpaulin-report.html`。
- [x] 結案報告 `missions/mission-unit-test-report.md`。
