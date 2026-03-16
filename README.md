# Just Keep Identity (jki)
> **Low-friction MFA & Identity Session Manager for CLI Power Users.**

![JKI Demo](docs/assets/demo.gif)

[繁體中文](README.zh-TW.md)

## 📖 The Backstory

I built JKI because I moved from Windows to macOS and couldn't find a 2FA manager that didn't annoy me. I was a long-time WinAuth user, but on macOS, everything was either behind a paywall, bloated with GUIs, or just felt slow.

For a developer, taking your hands off the home row to click a GUI search box feels like a bug. Finding an account should be a matter of milliseconds. JKI exists so I can get my OTP and get back to work without my hands ever leaving the keyboard.

I don't trust the cloud with my secrets. Why bother with another proprietary service when we already have Git? JKI uses your own infrastructure for syncing, while keeping secrets hardware-bound or strongly encrypted.

## 🚀 The JKI Way

*   **Zero Latency**: Fuzzy search and copy to clipboard in a few keystrokes.
*   **Hardware-Bound Security**: Your secrets stay in your OS-native vault (macOS Keychain / Linux Secret Service).
*   **Encryption-on-Transport**: JKI automatically encrypts secrets before they ever touch your Git staging area. No Master Key, no sync.
*   **Git for Metadata**: Use your own Git repo for syncing account structure. You control the infrastructure.
*   **Micro-Roll UX**: Optimized `j-k-i` command set designed for one-handed operation.
*   **Officially Signed**: macOS version is notarized by Apple to avoid Gatekeeper warnings.

## 🧬 Technical DNA

Built with Rust for performance and safety:

*   **Intelligent Agent**: `jki-agent` manages decrypted memory cache. It features an **automatic TTL (1-hour session)** that wipes secrets from memory after inactivity. *(Currently optimized for macOS)*.
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

# For Linux/Windows (Headless Agent):
./install.sh --headless
```

---

## 🛡 Security Architecture

JKI adopts a **"Hybrid Vault"** strategy to ensure maximum security without sacrificing portability:

| Component | Storage (Local) | Storage (Sync) | Security |
| :--- | :--- | :--- | :--- |
| **Identity Metadata** | Local File | Git / Repo | Publicly visible in repo |
| **OTP Secrets** | **OS Keyring** | **Encrypted Git** | AES-256 (Master Key required) |

### Why this design?
- **Zero Disk Leak**: Your actual secrets are never stored in plaintext on disk. They live in your OS-native vault (macOS Keychain / Linux Secret Service).
- **Auto-Hardening Sync**: When running `jkim git sync`, JKI intelligently detects plaintext secrets. If a Master Key is available, it will **automatically encrypt** them before staging, ensuring your secrets are always protected during transport.
- **Git as Your Cloud**: Why entrust your keys to a 3rd-party SaaS? Use your own Git infrastructure (GitHub, GitLab, or a private server) to sync metadata while keeping secrets encrypted.
- **Safe Syncing**: Even if your Git repository is compromised, the attacker only sees *who* you have accounts with. The actual keys are useless blobs without your Master Key.


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
