# JKI - Hacker News Launch Strategy

This document outlines the strategy for launching **Just Keep Identity (JKI)** on Hacker News (Show HN).

## 1. The "Show HN" Draft

**Title**: Show HN: JKI – A low-friction MFA manager for CLI power users, built in Rust

**Body**:

Hi HN,

I'm Li-chih Wu, and I've been working on **JKI (Just Keep Identity)**, a TOTP/MFA manager designed for engineers who live in the terminal.

### The Backstory
I've been a WinAuth user for years on Windows, but when I migrated to macOS, I found most of the alternatives were either paid apps or just didn't fit my workflow. 

I initially considered porting WinAuth to macOS, but then I realized: even if I did, the GUI approach was the bottleneck. Sifting through 30+ entries in a list, clicking search boxes, and manual typing is inherently slower than staying on the home row. I wanted something where finding an account was faster than the time it takes for my hands to leave the keyboard.

JKI was built to be a "Zero-Cloud" option. Why bother with another proprietary cloud service when we already have Git? JKI uses your own Git infrastructure (GitHub, or your private server) for syncing metadata, while keeping secrets pinned to your hardware.

### What makes it different?
- **Efficiency**: Search and copy to clipboard with minimal keystrokes. It uses a fuzzy search engine with character highlighting.
- **Security Model**: It splits data into **Metadata** (account names, issuers) which can be versioned in Git, and **Secrets** (TOTP keys) which stay in your OS-native Keyring (macOS Keychain / Linux Secret Service).
- **Smart Agent**: A background agent (`jki-agent`) handles decrypted memory cache with an automatic 1-hour TTL. On macOS, it's officially signed, notarized, and supports **TouchID** for seamless unlocking.
- **CLI Ergonomics**: Optimized for one-handed operation. Commands like `jki <pattern> <index>` make selecting from multiple accounts effortless.
- **Unix-Friendly**: Full pipe support (`stdout -`), integrates easily with your existing scripts.

### How to try it
- **macOS (Homebrew)**:
  ```bash
  brew tap lichih/jki
  brew install jki
  ```
- **Source**: `git clone https://github.com/lichih/just-keep-identity` (Requires Rust).

The macOS version is fully signed and notarized to avoid "unidentified developer" warnings. For Linux and Windows, we provide a lightweight CLI-only core.

I'd love to hear your feedback on the security model and the CLI workflow.

GitHub: [https://github.com/lichih/just-keep-identity](https://github.com/lichih/just-keep-identity)

---

## 2. Recommended README.md Tweaks (HN-Friendly)

Hacker News users appreciate factual, humble, and technical language.

| Current Phrase | Recommended HN Phrase | Reason |
| :--- | :--- | :--- |
| "Extreme speed" | "High-speed" or "Latency-optimized" | Avoids sounding like "marketing speak". |
| "Extreme stability" | "Built with Rust for safety" | Focuses on technical merit. |
| "Millisecond speeds" | "Low-friction workflow" | Focuses on efficiency rather than raw speed. |
| "Extreme Velocity" | "Low-latency workflow" | More professional. |

## 3. Pre-Launch Checklist

1. [ ] **Verify Release Assets**: Ensure `v0.1.0-alpha` has the notarized `.tar.gz` and binaries attached.
2. [ ] **TouchID Verification**: Confirm that `jki-agent` correctly triggers the TouchID prompt on a fresh install.
3. [ ] **Documentation**: Ensure the `README.md` demo GIF correctly shows the fuzzy search highlighting.
4. [ ] **Security Audit**: Run `./scripts/security-audit.sh` one last time.
5. [ ] **Username**: Use your personal account (`lichih`), not a brand account.
6. [ ] **Profile**: Ensure your email is in your HN profile (dang's recommendation).
