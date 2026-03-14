#!/bin/bash
# JKI Security Audit Script
# Purpose: Programmatic verification of security boundaries.

EXIT_CODE=0

echo "--- JKI Security Audit ---"

# 1. Verify .gitignore integrity
echo -n "Checking .gitignore for critical exclusions... "
CRITICAL_PATTERNS=("private/" "data/" "master.key" "vault.json" "config.json")
for pattern in "${CRITICAL_PATTERNS[@]}"; do
    if ! grep -q "^$pattern" .gitignore && ! grep -q "/$pattern" .gitignore; then
        echo -e "\n[ERROR] Missing critical exclusion: $pattern"
        EXIT_CODE=1
    fi
done
if [ $EXIT_CODE -eq 0 ]; then echo "OK"; fi

# 2. Check for untracked sensitive files
echo -n "Checking for untracked sensitive data... "
UNTRACKED=$(git status --porcelain | grep '??' || true)
if echo "$UNTRACKED" | grep -qE "private/|data/|master.key|vault.json"; then
    echo -e "\n[ERROR] Untracked sensitive files detected!"
    echo "$UNTRACKED"
    EXIT_CODE=1
else
    echo "OK"
fi

# 3. Verify git history for leaks (Brief scan)
echo -n "Scanning git objects for historical leaks... "
if git rev-list --objects --all | grep -qE "private/|master.key"; then
    echo -e "\n[ERROR] Potential leak found in git history!"
    EXIT_CODE=1
else
    echo "OK"
fi

if [ $EXIT_CODE -eq 0 ]; then
    echo "--- Result: SECURE ---"
else
    echo "--- Result: CRITICAL FAILURE ---"
fi

exit $EXIT_CODE
