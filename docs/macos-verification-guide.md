# macOS Binary Verification Guide

This guide provides commands to verify the authenticity and security status of JKI binaries.

## 1. Checking Code Signature
Use `codesign` to verify that the binary is signed by the official Developer ID.

```bash
# Basic check
codesign -dvvv target/release/jki

# Look for these lines in the output:
# Authority=Developer ID Application: Li-chih Wu (9G55ZALW6V)
# TeamIdentifier=9G55ZALW6V
# flags=0x10000(runtime)
```

## 2. Verifying Notarization (Gatekeeper)
To ensure the binary will pass macOS Gatekeeper checks on other machines:

```bash
# Verify notarization status
spctl -a -vvv target/release/jki

# Success output should show:
# source=Notarized Developer ID
```

## 3. Deep Verification (Bundle)
For the full `.app` bundle:

```bash
codesign --verify --verbose --deep target/release/jki-agent.app
```
