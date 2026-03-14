# Just Keep Identity (jki)
> **Extreme speed MFA & Identity Session Manager for CLI Power Users.**

![JKI Demo](docs/assets/demo.gif)

[繁體中文](README.zh-TW.md)

`jki` is an identity authorization tool designed specifically for engineers. It's not just about managing TOTP; it's about completing authentication at "millisecond" speeds without ever leaving your terminal.

## 🚀 Core Philosophy

*   **Extreme Velocity**: Search and copy in < 3ms. By the time you need the OTP, it's already in your clipboard.
*   **Fuzzy Intelligence**: Advanced fuzzy search with character highlighting. Locate accounts instantly even if you don't remember the exact name.
*   **Smart Agent**: Intelligent background agent supporting auto-unlock for plaintext vaults and active disk synchronization (Active Reload).
*   **Physical Isolation**: Built on OS Keyring. Your secrets stay in your system's secure enclave—zero cloud dependency.
*   **CLI Ergonomics**: Optimized Micro-Roll command set (`j-k-i`), allowing for one-handed operation.

## 🧬 Technical DNA

Built with Rust for extreme stability and security:

*   **Intelligent Agent**: `jki-agent` manages decrypted memory cache. It's the secure gateway to OS Keyring integration.
*   **Hybrid Vault**:
    *   **Metadata**: Managed via local files and Git for versioning.
    *   **Secrets**: Directly integrated with OS Keyring (macOS Keychain, Linux Secret Service).
*   **Unix-Friendly**: Perfect pipe support (`stdout -`), easily integrates with `ssh`, `git`, `kubectl`, and other CLI tools.

## 🛠 Quick Start

```bash
# Query and copy OTP (Priority: Agent -> Keyfile -> Password Prompt)
jki github

# Smart Filtering: Search for "google" and select the 2nd result
jki google 2

# Force List Mode: View matches without executing
jki google -l

# Fast Vault Sync (Git commit/pull/push)
jkim git sync
```

### Smart Filtering & Selection

`jki` follows a "Filter -> Action" logic chain, making it effortless to navigate complex account lists:

1.  **Multi-Pattern Filtering**: `jki [PATTERNS]... [INDEX]`
    *   `jki u`: Lists all accounts matching `u` (e.g., Uber, Uplay).
    *   `jki u 2`: Directly acquires the OTP for the 2nd item in the results.
2.  **List Mode (`-l, --list`)**:
    *   Appended `-l` switches `jki` to "View Only" mode.
    *   Extremely useful for verifying index numbers in large result sets.
3.  **Graceful Feedback**: Ambiguous results are no longer errors; JKI elegantly lists candidates with score gaps to guide your next keystroke.

---

## 📦 Installation

### Option A: Homebrew (Recommended for macOS)
```bash
brew tap lichih/jki
brew install jki
```

### Option B: From Source (For Developers/Linux)
Ensure you have the [Rust toolchain](https://rustup.rs/) installed:
```bash
git clone https://github.com/lichih/just-keep-identity.git
cd just-keep-identity
make install
```

---

## 🛡 Security Architecture & Mental Model

JKI adopts a **"Separation of Concerns"** strategy to ensure maximum security without sacrificing portability:

| Component | Storage Type | Content | Portability |
| :--- | :--- | :--- | :--- |
| **Identity Metadata** | Git / Local File | Account names, Issuers, Indexing | **High** (Sync via Git) |
| **OTP Secrets** | OS Keyring | The actual TOTP Secret Keys | **Zero** (Locked to Hardware) |

### Why this design?
- **Zero Disk Leak**: Your actual secrets are never written to disk in plaintext. They are stored in your OS-native vault (macOS Keychain / Linux Secret Service).
- **Safe Syncing**: You can safely push your JKI Git repository to a private cloud. Even if the repo is compromised, the attacker only sees *who* you have accounts with, not the *keys* to access them.

## 🔄 Syncing & Disaster Recovery

### Setting up a New Machine
1. `git clone` your JKI repository to the new machine.
2. Run `jkim git sync` to restore your account structure.
3. **Important**: You must manually re-add the secrets for each account using `jkim add -f <account>`. Metadata travels via Git; Secrets do not.

### Disaster Recovery Plan
- **Backup**: We recommend keeping your original 2FA Recovery Codes in a separate, offline location (e.g., a physical safe).
- **Recovery**: If you lose access to your OS Keyring (e.g., system wipe without backup), use your Recovery Codes to reset your 2FA and re-add them to JKI.

---

*Built with ❤️ for those who live in the terminal.*
