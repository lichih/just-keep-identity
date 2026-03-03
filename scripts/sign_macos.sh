#!/bin/bash
set -e

APP_BUNDLE=$1
SIGNING_IDENTITY=${2:-"Developer ID Application: Your Name (TeamID)"}
ENTITLEMENTS="crates/jki-agent/jki.entitlements"

if [ -z "$APP_BUNDLE" ]; then
    echo "Usage: $0 <path-to-app-bundle> [signing-identity]"
    exit 1
fi

echo "Signing $APP_BUNDLE with identity: $SIGNING_IDENTITY"

# Sign the binary within the bundle first
codesign --force --options runtime --entitlements "$ENTITLEMENTS" --sign "$SIGNING_IDENTITY" "$APP_BUNDLE/Contents/MacOS/jki-agent"

# Sign the entire bundle
codesign --force --options runtime --entitlements "$ENTITLEMENTS" --sign "$SIGNING_IDENTITY" "$APP_BUNDLE"

echo "Signing complete. Verifying signature..."
codesign --verify --verbose --deep "$APP_BUNDLE"
