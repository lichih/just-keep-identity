# JKI - Hacker News Submission Strategy (The WinAuth Spirit)

## Step 1: Submit Link
- **URL**: `https://jki.4649.tw`
- **Title**: `Show HN: JKI – Getting auth codes should never require a mouse`

## Step 2: Immediate First Comment

Hi HN,

I built JKI because I moved from Windows to macOS and couldn't find a 2FA manager that didn't annoy me. I was a long-time **WinAuth** user. On macOS, I found that most tools were either behind a paywall, bloated with GUIs, or just painfully slow.

For a developer, taking your hands off the home row to click a GUI search box feels like a bug. Finding an account should be a matter of milliseconds. JKI exists so I can get my OTP and get back to work without my hands ever leaving the keyboard.

**Key Technical Details:**
- **Zero Latency**: Fuzzy search and copy to clipboard in a few keystrokes.
- **Hardware Security**: Your secrets stay in your OS-native vault (macOS Keychain / Linux Secret Service).
- **Git-Synced**: Sync metadata and encrypted secrets via your own Git infrastructure. No proprietary cloud.
- **Rust Core**: Built for memory safety and zero-cold-start performance.

I'd love to hear how other CLI power users manage their MFA tokens and whether you've found a better way to stay on the home row.

**GitHub**: https://github.com/lichih/just-keep-identity
**Install (macOS)**: `brew tap lichih/jki && brew install jki`
