# Asset: JKI System Health & Status Guide
ID: @JKI_ASSET(guide_status)

The `jkim status` command provides a quick overview of your JKI installation's health.

## Key Indicators
- **Vault Status**: Shows if your metadata and encrypted/plaintext secrets are in sync.
- **Agent Status**: Indicates if the `jki-agent` is running and holding your master key.
- **Git Sync**: Reports if your local changes are committed and pushed to the remote repository.

## Recommended Actions
- If the **Agent** is stopped, run: `jkim agent start`
- If **Git** is not clean, run: `jkim sync`
- If **Master Key** is missing from Keychain, run: `jkim keychain push`
