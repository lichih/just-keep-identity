# JKI - Hacker News Submission Draft (Official)

**Title**: Show HN: JKI – A local-first, Git-synced MFA manager for the terminal

**Body**:

Hi HN,

I built JKI because I moved from Windows to macOS and couldn't find an MFA manager that fit my workflow. I was a long-time WinAuth user, but on macOS, everything was either behind a paywall or just felt slow.

Even with a GUI search, the friction of taking hands off the keyboard to click a box and type felt wrong for a CLI user. I wanted something where finding an account was faster than the time it takes for my hands to leave the home row.

**How it works:**
It splits your data: **Metadata** (account names/issuers) is stored in a local Git repo for easy syncing across your own machines, while **Secrets** are pinned to your hardware via OS Keyring (macOS Keychain / Linux Secret Service). No proprietary cloud, no subscription.

**Key Tech:**
- Built in Rust for safety and performance.
- Fuzzy search engine with character highlighting.
- Background agent with a 1-hour memory TTL (supports TouchID on macOS).
- Tiered distribution: Signed/Notarized app for macOS; lightweight headless daemon for Linux/Windows.

I’d love to hear your thoughts on the security model and whether you still use GUI apps for 2FA.

GitHub: https://github.com/lichih/just-keep-identity

---

## Technical Appendix (Author's first comment)

If you're interested in the details:
- **Zero Disk Leak**: Actual TOTP secrets are never written to disk in plaintext.
- **Auto-Hardening**: When you sync with Git, JKI detects plaintext secrets and forces encryption via `age` before staging.
- **Latency**: The search engine is optimized for speed, returning results and copying to clipboard in milliseconds.
- **Homebrew**: `brew tap lichih/jki && brew install jki`
