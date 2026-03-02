# Mission: Just Keep Identity (jki) Unit Testing - Phase 2 (Hardened Paths & Integration)

## 1. Context Update
The project has transitioned to a split-data architecture:
- `vault.metadata.json`: Plaintext metadata.
- `vault.secrets.bin.age`: Encrypted secrets.
- `paths.rs`: Refactored to handle absolute paths and `JKI_HOME` robustly.

## 2. Objective
Maintain > 90% logic coverage while covering the new path resolution and data integration logic.

## 3. Key Logic to Test
### jki-core (New Targets)
- **Path Resolution**: 
    - Test `home_dir()` with and without `JKI_HOME`.
    - Test `metadata_path()` and `secrets_path()` env overrides.
    - **Note**: Use `serial_test` crate if env var manipulation causes race conditions during parallel tests.
- **Permission Checks**: Ensure `check_secure_permissions` correctly flags 0644 vs 0600 on Unix.
- **Git Utilities**: Expand `git::check_status` tests to cover different repo states (dirty, branch names).

### jki (Integration Logic)
- **Data Consistency**:
    - Mock a scenario where Metadata contains an ID missing from Secrets.
    - Test the default warning output vs. the `-q` quiet filtering behavior.
- **Standalone Flow**: Test the full cycle of `acquire_master_key` -> `decrypt` -> `integrate` (mocking the interactive prompt if possible).

### jkim (New Targets)
- **Initialization**: Verify `jkim init` creates the correct directory structure and template files.
- **Import Deduplication**: Ensure the "Read -> Decrypt -> Merge -> Encrypt" cycle in `import-winauth` is stable.

## 4. Technical Requirements
- **Mocking Filesystem**: Use `tempfile` extensively to avoid polluting the user's real `~/.config/jki`.
- **Environment Isolation**: Ensure tests don't leak `JKI_HOME` to each other.
- **Binary/Age Mocking**: Test the crypto functions with known test keys.

## 5. Handover Note
Refer to `missions/mission-unit-test-report.md` for current coverage baseline. Focus on the 0% coverage areas identified in the last report (especially `paths.rs` and `jkim`).

---
*Updated by Gemini-CLI for handover.*
