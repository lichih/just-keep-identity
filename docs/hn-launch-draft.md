# JKI - Hacker News Submission Strategy (Standard Link Post)

This is the recommended way to post on HN to ensure the "Show HN" link goes directly to your GitHub repository.

## Step 1: Submit Link
- **URL**: `https://github.com/lichih/just-keep-identity`
- **Title**: `Show HN: JKI – A local-first, Git-synced MFA manager for the terminal`

## Step 2: Immediate First Comment
*Post this immediately after submitting the link to provide context and seed the discussion.*

Hi HN,

I built JKI because I moved from Windows to macOS and couldn't find an MFA manager that fit my workflow. I was a long-time WinAuth user, but on macOS, everything was either behind a paywall or just felt too slow.

Even with a GUI search, the friction of taking hands off the keyboard to click a box and type felt wrong for a CLI user. I wanted something where finding an account was faster than the time it takes for my hands to leave the home row.

**How it works:**
JKI adopts a "Separation of Concerns" strategy:
- **Metadata** (account names/issuers) is stored in a local Git repo. You can sync it across your machines using your own infrastructure (GitHub, GitLab, or a private server).
- **Secrets** (TOTP keys) are pinned to your hardware. They stay in your OS-native Keyring (macOS Keychain / Linux Secret Service) and never touch the disk in plaintext.

**Key Technical Details:**
- **Zero-Cloud**: Why bother with another proprietary cloud service? Use Git as your cloud.
- **Auto-Hardening**: When you sync, JKI detects plaintext secrets and forces encryption via `age` before staging.
- **Smart Agent**: A background agent (`jki-agent`) handles decrypted memory cache with a 1-hour session TTL. On macOS, it's signed/notarized and supports TouchID.
- **Headless Mode**: For Linux and Windows, it can be compiled as a lightweight headless daemon without GUI dependencies.

I'd love to hear your feedback on the security model and how you currently manage your MFA tokens.

**GitHub**: https://github.com/lichih/just-keep-identity
**Install (macOS)**: `brew tap lichih/jki && brew install jki`
