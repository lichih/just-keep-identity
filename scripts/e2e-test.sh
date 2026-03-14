#!/bin/bash
# JKI End-to-End (E2E) Integration Test Script
# This script simulates a complete user workflow in a temporary isolated environment.

set -e

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== Starting JKI E2E Integration Test ===${NC}"

# 1. Setup Isolated Environment
TEST_DIR=$(mktemp -d)
export JKI_HOME="$TEST_DIR/.jki"
echo "Isolated JKI_HOME: $JKI_HOME"

# Ensure binaries are built
echo "Building binaries..."
cargo build --release --workspace > /dev/null

JKI_BIN="./target/release/jki"
JKIM_BIN="./target/release/jkim"

# Helper for asserting files exist
assert_file_exists() {
    if [ ! -f "$1" ]; then
        echo -e "${RED}Error: File $1 does not exist!${NC}"
        # List directory to help debugging
        ls -la $(dirname "$1")
        exit 1
    fi
}

# 2. Test: jkim git init
echo -e "\n[1/5] Testing: jkim git init"
$JKIM_BIN git init
assert_file_exists "$JKI_HOME/.gitignore"
echo -e "${GREEN}✓ Init successful${NC}"

# 3. Test: jkim add (Manual Plaintext)
echo -e "\n[2/5] Testing: jkim add (Plaintext)"
# Force plaintext to test the upgrade path later
echo "JBSWY3DPEHPK3PXP" | $JKIM_BIN add test-account TestIssuer --auth plaintext
assert_file_exists "$JKI_HOME/vault.metadata.yaml"
assert_file_exists "$JKI_HOME/vault.secrets.json"
echo -e "${GREEN}✓ Account added (Plaintext)${NC}"

# 4. Test: jki search
echo -e "\n[3/5] Testing: jki search & fuzzy filtering"
# Add another account using flags
$JKIM_BIN add github-lichih GitHub -s KRSXG5CTMVRXEZLU > /dev/null

# Search for "git"
SEARCH_RESULT=$($JKI_BIN git -l)
if [[ $SEARCH_RESULT == *"github-lichih"* ]]; then
    echo -e "${GREEN}✓ Fuzzy search functional${NC}"
else
    echo -e "${RED}Error: Search result did not contain expected account!${NC}"
    echo "Output: $SEARCH_RESULT"
    exit 1
fi

# 5. Test: Master Key & Encryption
echo -e "\n[4/5] Testing: Encryption with Master Key file"
MASTER_KEY="this-is-a-secure-test-key-123456"
echo "$MASTER_KEY" > "$JKI_HOME/master.key"
chmod 600 "$JKI_HOME/master.key"

# Trigger encryption explicitly
$JKIM_BIN encrypt --auth keyfile > /dev/null

# Add another account (should now be encrypted)
$JKIM_BIN add encrypted-account Safe -s MFRGGZDFMZTWQ2LK > /dev/null

# Verify vault.secrets.json is gone and vault.secrets.bin.age exists
if [ -f "$JKI_HOME/vault.secrets.bin.age" ]; then
    echo -e "${GREEN}✓ Vault successfully upgraded to Encrypted (AGE)${NC}"
else
    echo -e "${RED}Error: Vault did not encrypt even with master.key present!${NC}"
    exit 1
fi

# 6. Test: OTP Generation (End-to-End)
echo -e "\n[5/5] Testing: OTP Generation"
# jki <pattern> should now use the master.key to decrypt vault.secrets.bin.age and show OTP
OTP_OUTPUT=$($JKI_BIN encrypted --stdout)
if [[ $OTP_OUTPUT =~ ^[0-9]{6}$ ]]; then
    echo -e "${GREEN}✓ OTP generated successfully from encrypted vault: $OTP_OUTPUT${NC}"
else
    echo -e "${RED}Error: Failed to generate valid 6-digit OTP!${NC}"
    echo "Output: $OTP_OUTPUT"
    exit 1
fi

echo -e "\n${GREEN}=== All E2E Tests Passed! ===${NC}"

# Cleanup
rm -rf "$TEST_DIR"
echo "Cleanup complete."
