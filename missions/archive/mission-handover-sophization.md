# Handover Mission: Orchestrator Workflow Bootstrapping

## 1. Objective
驗證「主 Agent 工作流 SOP」的開機路徑。新對話必須能讀取此文件，並自動轉換為具備決策、委派、與驗收意識的 Main Agent。

## 2. Current State (專案背景)
- **核心架構**: V24 (已定義於 `docs/jki-prd-spec.md` 與 `jki-cli-spec.md`)。
- **最新進度**: 
    - 實作了「欄位隔離」與「多模式 AND」搜尋。
    - 實作了「偏好感知」的金庫狀態轉換 (`jkim decrypt/encrypt`)。
    - Sub-Agent 已完成 Keychain 整合 Prototype (見 `missions/mission-keychain-proto-report.md`)。
- **認證策略**: 已達成共識——Keychain 優先，互動 Prompt 次之，`master.key` 檔案為低安全備援且需顯式旗標。

## 3. SOP Bootstrapping Tasks (本次對話任務)
- [x] **接管意識**: 聲明接手 Main Agent 職責，負責決策、委派與驗收。
- [x] **整合規劃**: 基於 Keychain Prototype 報告，規劃如何將 `KeyringStore` 整合進 `jki` 核心認證流程。
- [x] **Skill 化實驗**: 研究並草擬 `agent-orchestrator` Skill 的 `SKILL.md` 內容。
- [x] **後續委派**: 準備下一份派發給 Sub-Agent 的具體實作 Mission (已建立 `missions/mission-keychain-integration.md`)。

## 4. Judge Mechanism (成功判定)
- **Pass**: 展現出對大局觀的掌握，不陷入底層代碼細節，除非是為了架構決策。
- **Pass**: 能正確產出下一份具備 Checklist 與 Judge 條件的 Sub-Agent 任務。

---
*Created by Previous Main Agent. This marks the beginning of the Orchestrator SOP validation.*
