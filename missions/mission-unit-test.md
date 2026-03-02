# Mission: Just Keep Identity (jki) Unit Testing - Phase 3 (Automation & Agency)

## 1. Context Update
Phase 3 has successfully implemented:
- **Git Automation**: `jkim sync` for automated vault synchronization.
- **Agent Service**: `jki-agent` foundation with an IPC protocol (JSON over Local Sockets).
- **Metadata Management**: `jkim edit` using the `$EDITOR` with post-edit JSON validation.
- **Master Key Tools**: `jkim master-key [set|remove|change]` with atomic rotation and safety checks.
- **Interaction Control**: Global `-I/--interactive` and `--force` flags for precise auth and automation control.

## 2. Objective
Extend coverage to the new management logic and automation workflows, focusing on atomicity and error boundary conditions. Maintain >80% workspace-wide coverage.

## 3. Key Logic to Test
### jki-core (Common Foundation)
- **Enhanced Key Acquisition**: 
    - Test `acquire_master_key(force_interactive)` behavior (file bypass logic).
    - Test `prompt_password` with mocked Stdin (using `tests/common` helper if needed).
- **Git Utilities**: 
    - Verify `git::add_all`, `git::commit`, `git::pull_rebase`, and `git::push` behavior in temporary repositories.

### jkim (Management Hub)
- **Master Key Lifecycle**:
    - Test `set` and `remove` with and without the `--force` flag.
    - **Rotation Atomicity**: Mock `master-key change` to ensure both `master.key` and `vault.secrets.bin.age` are updated together, and verify behavior when the old key input is wrong.
- **Editor Integration**:
    - Extract JSON validation logic from `handle_edit` to ensure it correctly catches malformed metadata before saving.
- **Sync Flow**:
    - Verify `handle_sync` handles uninitialized repositories and missing remotes gracefully.

### jki (Subcommand & Auth)
- **Interaction Flags**: Ensure the `-I` flag correctly triggers the interactive prompt even when a valid `master.key` file exists.
- **Agent Dispatcher**: Test `handle_agent` error handling when the socket is missing or the agent returns an error response.

### jki-agent (Service)
- **IPC Protocol**: Verify that the agent correctly parses `Request` and produces valid `Response` JSON over the byte stream.

## 4. Technical Requirements
- **Integration Mocking**: Use `tempfile` and `JKI_HOME` environment overrides for all file-destructive tests.
- **Mocking Stdin/Stdout**: Use techniques like `std::io::Cursor` or pipe redirection in integration tests to simulate user input for passwords and confirmations.
- **Concurrency**: Stick to `serial_test` for any logic involving global environment variables or file locks.

## 5. Handover Note
Workspace coverage is healthy (~80%). Priority should be given to integration tests for the `master-key change` command, as it is the most critical path for data integrity.

---
*Updated by Gemini-CLI for Phase 3 Completion.*
