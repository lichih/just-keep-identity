# Mission Report: jki-agent Startup Validation

## 1. Objective
確保 `jki-agent` 在啟動時根據旗標進行環境驗證。特別是在 `--force-age` 模式下，若加密金庫缺失應立即終止，而非進入監聽狀態。

## 2. Changes
- 修改 `crates/jki-agent/src/main.rs`:
    - 在 `main` 函式中，於 `LocalSocketListener::bind` 之前加入檢查：
    - 若 `force_age` 為 true 且 `JkiPath::secrets_path()` 不存在：
        - 輸出錯誤訊息：「CRITICAL: Force-age mode enabled but encrypted vault (.age) is missing. Exit.」
        - 呼叫 `std::process::exit(1)`。

## 3. Verification Result

### 3.1. Case 1: Force-age mode enabled, .age missing
- **Environment**: 僅存在 `.json` 檔案。
- **Command**: `jki-agent --force-age`
- **Result**: 程式輸出錯誤並終止。
- **Log**: 
```text
CRITICAL: Force-age mode enabled but encrypted vault (.age) is missing. Exit.
Exit Code: 1
```

### 3.2. Case 2: Force-age mode enabled, .age exists
- **Environment**: 存在 `.age` 檔案。
- **Command**: `jki-agent --force-age`
- **Result**: 程式正常啟動並進入監聽狀態。
- **Log**: 
```text
jki-agent listening on "/Users/lichih/code/just-keep-identity/temp_test_home/jki.sock" (force_age: true)
```

### 3.3. Case 3: Force-age mode disabled, only .json exists
- **Environment**: 僅存在 `.json` 檔案。
- **Command**: `jki-agent`
- **Result**: 程式正常啟動。
- **Log**: 
```text
jki-agent listening on "/Users/lichih/code/just-keep-identity/temp_test_home/jki.sock" (force_age: false)
```

## 4. Conclusion
任務成功完成。`jki-agent` 現在具備啟動環境驗證機制，防止在強制加密模式下誤啟動。
