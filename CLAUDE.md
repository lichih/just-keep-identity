# Just Keep Identity (JKI) - Project & Agent Mandates

This document establishes the absolute engineering principles and agent workflows for JKI to ensure **Single Source of Truth (SSoT)**.

## 1. Core Mandates (from GEMINI.md)

### 1.1 Authentication & Automation
To ensure tests and CI/CD are not interrupted by OS authorization prompts (e.g., macOS Keychain ACL), the Master Key acquisition priority is fixed:
1. **Master Key File (`master.key`)**: Priority 1.
2. **Agent Session**: Request from the background agent.
3. **System Keyring**: Final fallback.

**Any unit test involving keys must pass "silently" if a physical key file exists.**

### 1.2 Defensive CLI Design
- **Authorization & Quiet Mode**: Any changes to flag behavior (especially `-f`, `-d`, `-q`) **MUST** strictly follow the "Authorization & Suppression Matrix" in `docs/jki-cli-spec.md` (Chapter 1.1).
- **Quiet Mode (`-q`)**: 
  - On failure: MUST print clear error to `stderr`.
  - On success: MUST stay completely silent.
- **Force Mode (`-f`)**: `add -f` means "Force Add" (generate new UUID). **NEVER** perform auto-overwrite to protect physical data integrity.

### 1.3 Physical Integrity
- **Hidden Input**: Secret inputs in TTY mode must use masks.
- **Normalization**: Secrets must be `trim()`, `replace(" ", "")`, and `to_uppercase()` before being saved to physical storage.

## 2. Agent Workflows (Opencode Native)

### 2.1 Engineering Specifics
- **Stable Sorting Rule**: Intelligence features (highlighting, auto-selection) must NOT disrupt the stable vault-order indexing.
- **Diagnostics**: Prioritize feedback transparency (e.g., showing score gaps in ambiguous matches).
- **Tooling**: Authorized to use `make release`, `make install`, and `make test-all` for verification. Use `make cov` for accurate coverage reports via `llvm-cov`. Use `codesign -dvvv <bin>` to verify signatures.

### 2.2 Data Access Privileges
- **Dynamic Visibility**: Respect `.gitignore` to avoid reading unnecessary or large binary files (e.g., `target/`).
- **Anti-Ignore Logic**: Explicitly authorized to use `.geminiignore` (or `.agentignore`) as an "allow-list" to read files ignored by git but necessary for development (e.g., `data/private/`, `*.stable`).
- **Safety**: Never include contents from ignored or private directories in git commits.

### 2.5 Strategic Edit Mandates (Atomic Edits)
- **Coordinate-Anchored Edit (CAE)**: The `edit` tool now **requires** `startLine` and `endLine`. The agent MUST use the `Read` tool first to verify the exact physical coordinates.
- **Coordination Rule**: To avoid counting errors (LLM limitation), the agent MUST use the physical line numbers from the `Read` tool to define the range and verify the bit-by-bit content.
- **Checksum Logic**: The tool automatically performs a strict comparison. If the content at `[startLine, endLine]` doesn't match `oldString` exactly (including whitespace), the edit will fail.
- **Safety Guard**: The tool will block edits that remove sensitive patterns (like `# <SECURE>`, `# Private`) unless they are explicitly preserved in the `newString`.
- **Diagnostics**: If an edit fails or succeeds, the tool provides physical coordinates and hints about format mismatches (like indentation or line endings) to help you correct your intent.
