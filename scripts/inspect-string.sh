#!/bin/bash
# JKI String Inspector
# Helps the agent verify exact line counts and invisible characters before editing.

FILE=$1
TARGET_STRING=$2

if [ ! -f "$FILE" ]; then
    echo "Usage: $0 <file_path> <string_to_inspect>"
    exit 1
fi

echo "--- String Inspection for: $FILE ---"

# 1. Use grep to find the line range
# -F for fixed string, -n for line numbers
MATCH_DATA=$(grep -Fn "$TARGET_STRING" "$FILE" || true)

if [ -z "$MATCH_DATA" ]; then
    echo "[ERROR] String not found in file."
    exit 1
fi

LINE_START=$(echo "$MATCH_DATA" | head -n 1 | cut -d: -f1)
LINE_COUNT=$(echo -n "$TARGET_STRING" | grep -c '^' || echo 1)
# Correct count if target ends with newline
if [[ "$TARGET_STRING" == *$'\n' ]]; then
    ((LINE_COUNT++))
fi
LINE_END=$((LINE_START + LINE_COUNT - 1))

echo "Found at: Lines $LINE_START to $LINE_END"
echo "Total expectedOldLineCount: $LINE_COUNT"

# 2. Check for invisible characters at the end
echo -n "Invisible chars check: "
if [[ "$TARGET_STRING" == *$'\r' ]]; then
    echo "CRLF detected (Windows style)."
elif [[ "$TARGET_STRING" == *$'\n' ]]; then
    echo "Trailing newline detected (LF)."
else
    echo "No trailing newline."
fi

# 3. Show raw visualization (like cat -e)
echo "Raw Preview (with line endings):"
echo -n "$TARGET_STRING" | cat -e
echo ""
