# Just Keep Identity (JKI) - User Manual
ID: @JKI_ASSET(guide_man)

`jki` is a minimalist, high-speed identity management suite designed for power users.

## Core Philosophy
1. **Speed**: Sub-3ms cold start.
2. **Minimalism**: Focus on search and OTP generation.
3. **Integrity**: Single Source of Truth for metadata and secrets.

## Common Workflows

### 1. Daily Usage (jki)
Search and generate OTPs with fuzzy matching:
```bash
jki ggl 2  # Match Google, select the 2nd entry
```

### 2. Management (jkim)
Add new accounts or sync with Git:
```bash
jkim add user@gmail.com Google
jkim sync
```

### 3. Cleanup (dedupe)
Find and remove duplicate secret entries:
```bash
jkim dedupe
```

## Security
- Secrets are encrypted using **age** (X25519 or Passphrase).
- Master key can be stored in the **System Keychain**.
- Background agent (`jki-agent`) provides secure memory caching.
