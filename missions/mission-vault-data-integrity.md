# Mission: Vault Data Integrity & YAML Migration

## 1. 背景與目標 (Background & Objective)
目前的 metadata 採用 JSON 格式，對手動編輯不友善且容易發生語法錯誤。此外，`jkim import` 在處理非唯一 Secret 時缺乏穩定的識別邏輯，存在標籤誤蓋 (Label Overwriting) 的風險。

本任務目標是：
- **格式遷移**：從 `vault.metadata.json` 全面遷移至 `vault.metadata.yaml`。
- **配置檢核**：新增 `jkim config check` 指令，取代舊有的 `jkim edit` 暫存檔機制。
- **匯入硬化**：修正 `jkim import` 邏輯，確保在 Secret 不唯一時執行安全新增，而非自動誤蓋現有條目。
- **告知義務**：在文件與 README 中明確定義 Git 同步衝突的處理紀律。

## 2. 策略與階段 (Strategy & Phases)

### Phase 1: YAML 遷移與單一真理 (YAML SSoT)
1. **格式遷移**：將系統權威元數據來源改為 `vault.metadata.yaml`。
   - 優點：支援註解、人類可讀性高、Git 合併衝突較易人工修正。
2. **具體格式範例 (`vault.metadata.yaml`)**：
   ```yaml
   # Just Keep Identity Metadata - V1
   version: 1
   accounts:
     # 專案 A 的 2FA 帳號
     - id: "uuid-789"
       name: "user@project-a.com"
       issuer: "GitHub"
       account_type: "Standard"

     # 私人測試帳號 (備註：這是測試用的)
     - id: "uuid-456"
       name: "admin"
       issuer: "Localhost"
       account_type: "Standard"
   ```
3. **新增 `jkim config check`**：
   - 語法校驗：檢查 YAML 結構。
   - 一致性檢查：確保 Metadata 裡的 ID 在加密金庫中皆有對應秘密（無孤立 ID）。
   - 安全性：檢查實體檔案權限是否維持 0600。

### Phase 2: 匯入邏輯硬化 (Importer Hardening)
1. **移除不安全映射**：將 `secret_to_id` 反查表改為 `HashMap<String, Vec<String>>`（一對多）。
2. **三元組嚴格比對**：
   - 優先比對 `(Issuer + Name + Secret)`。若完全一致，視為重複帳號（跳過）。
   - 若標籤不同但 `Secret` 在金庫中為「唯一」：可執行自動更新標籤。
   - 若 `Secret` 在金庫中為「非唯一」：**禁止自動更新**。執行安全新增（產生新 UUID）並輸出 `[Ambiguous]` 警告。

### Phase 3: 告知義務與文件 (Documentation)
1. **README 更新**：新增「Git 同步最佳實踐 (Sync-First Rule)」專章。
   - **Sync-First 律令**：操作前必先 `jkim sync`。
2. **自動備份與救援流程 (Conflict Rescue)**：
   - **自動備份**：當 `jkim sync` 偵測到 `.age` 二進位衝突時，系統會自動將本地的 `vault.secrets.bin.age` 複製為 `vault.secrets.bin.age.conflict_[TIMESTAMP]`。
   - **救援指引**：在 stderr 輸出救援指令：
     ```text
     [CONFLICT] Binary secret vault collided! 
     [SAFEGUARD] Local copy backed up to: ./vault.secrets.bin.age.conflict_20260306
     [RESCUE] 
       1. Finish sync (accept remote version).
       2. Run 'jkim decrypt --source vault.secrets.bin.age.conflict_...'.
       3. Run 'jkim import-winauth vault.secrets.json' to re-merge lost keys.
     ```
3. **PRD 更新**：在 `docs/jki-prd-spec.md` 新增「帳號唯一性與去重準則」章節。

## 3. 完成定義 (Definition of Done)
- [ ] `vault.metadata.yaml` 成為系統唯一元數據來源。
- [ ] `jkim config check` 可精準識別語法錯誤與孤魂野鬼 ID。
- [ ] `jkim import-winauth` 不再因 Secret 碰撞而誤蓋帳號標籤。
- [ ] 已在 README 中明確告知 Git 同步衝突的處理流程與「Sync-First」紀律。
