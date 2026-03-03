#!/bin/bash
set -e

APP_BUNDLE=$1
APPLE_ID=${2:-"your-apple-id@example.com"}
TEAM_ID=${3:-"YOURTEAMID"}
PASSWORD=${4:-"@keychain:AC_PASSWORD"} # Use keychain reference for security

if [ -z "$APP_BUNDLE" ]; then
    echo "Usage: $0 <path-to-app-bundle> [apple-id] [team-id] [app-specific-password]"
    exit 1
fi

ZIP_FILE="jki-agent.zip"
echo "Creating zip for notarization..."
/usr/bin/ditto -c -k --keepParent "$APP_BUNDLE" "$ZIP_FILE"

echo "Submitting to Apple for notarization..."
xcrun notarytool submit "$ZIP_FILE" --apple-id "$APPLE_ID" --team-id "$TEAM_ID" --password "$PASSWORD" --wait

echo "Notarization complete. Stapling ticket..."
xcrun stapler staple "$APP_BUNDLE"

echo "Verification:"
spctl --assess --verbose --type exec "$APP_BUNDLE"

rm "$ZIP_FILE"
