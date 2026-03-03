# Mission: Keychain Integration Research & Prototype

## 1. Objective
研究 `keyring` crate 並實作一個迷你的 Prototype，驗證 JKI 能否在 macOS (Keychain) 與 Windows (DPAPI) 安全地存取 Master Key。

## 2. Fixed Requirements (Mandatory)
- [x] **Dependency Audit**: 確認 `keyring` crate 的安全性與相依性。
- [x] **Cross-Platform**: 程式碼必須能同時在 macOS 與 Windows 上編譯（或透過 cfg 隔離）。
- [x] **Unit Tests**: 必須包含模擬金鑰存取的測試案例。
- [x] **Closure Report**: 必須依照 `Completion Schema` 提供結案報告。

## 3. Implementation Checklist
- [x] 撰寫 `examples/keyring_proto.rs` 驗證以下行為：
    - [x] `Set`: 存入一筆 Service="jki", User="master-key" 的密碼。
    - [x] `Get`: 成功取回密碼。
    - [x] `Delete`: 成功刪除密碼。
- [x] 評估 `jki-core` 應該新增哪些 Trait 來封裝 Keychain 的差異。

## 4. Judge Mechanism (Success Criteria)
- **Hard Pass**: `cargo run --example keyring_proto` 在目前平台執行成功。
- **Hard Pass**: 所有新增的測試通過。
- **Fail Condition**: 報告中漏掉 Checklist 中任何一項的執行說明。
- **Fail Condition**: 將 Secret 打印到 stdout（除非是為了 Debug 且報告中註明已移除）。

## 5. Completion Schema (結案報告格式)
1. **Summary**: 實作了什麼。
2. **Evidence**: `cargo test` 與範例執行的輸出。
3. **Checklist Status**: 逐項標註 [OK] 並簡述做法。
4. **Spec Impact**: 對 `docs/jki-cli-spec.md` 的影響建議。

---
*Created by Main Agent. Sub-Agent should read this and start working in the new window.*
