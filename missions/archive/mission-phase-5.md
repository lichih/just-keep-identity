# Mission: Phase 5 - Productization & Architectural Cleanup

## 1. Objective
優化 JKI 產品體驗，並執行「架構減法」。核心目標是移除無意義的二進位優化規劃，轉而強化系統的同步穩定性與安全性。

## 2. Fixed Requirements (Mandatory)
- [ ] **Spec Cleanup**: 從所有文件中移除關於 `rkyv` 與二進位優化的描述。
- [ ] **Unit Tests**: 針對新實作的匯出與衝突處理邏輯編寫測試。
- [ ] **Closure Report**: 必須包含清理後的規格對照表。

## 3. Implementation Checklist
- [ ] **Docs Cleanup**: 
    - 修改 `docs/jki-prd-spec.md`：重新定義 Phase 5 為 Productization & Reliability。
    - 檢查所有文件中是否還有過時的二進位格式描述。
- [ ] **Sync UX Refinement**: 
    - 規劃 `jkim sync` 的衝突處理流程。
    - 利用 `confirm(..., default=true)` 實作自動化建議路徑（例如：預設保留本地修改）。
- [ ] **Secure Export (`jkim export`)**: 
    - 實作匯出功能：將金庫轉為 `otpauth` URI 格式並寫入加密 ZIP。
    - 需支援自訂匯出密碼。
- [ ] **Installation Script**: 
    - 提供一個 `Makefile` 或 `install.sh` 協助自動編譯並設置 PATH。

## 4. Judge Mechanism
- **Hard Pass**: `grep -r "rkyv" .` 在清理後不應有實質結果。
- **Hard Pass**: 實測 `jkim export` 產出的 ZIP 能在外部環境解密並讀取內容。

---
*Created by Main Agent. This is the official goal for Phase 5.*
