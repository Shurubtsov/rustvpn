#!/bin/bash
# PostToolUse hook: lint Rust files after edit
CHANGED_FILE="${CLAUDE_TOOL_ARG_FILE_PATH:-}"
if [[ "$CHANGED_FILE" == *.rs ]]; then
    cd "$(dirname "$0")/../../src-tauri" 2>/dev/null || exit 0
    cargo fmt --check 2>/dev/null
    if [ $? -ne 0 ]; then
        echo "⚠ Rust formatting issues detected. Run: cargo fmt"
    fi
fi
