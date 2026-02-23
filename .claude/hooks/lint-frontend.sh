#!/bin/bash
# PostToolUse hook: lint frontend files after edit
CHANGED_FILE="${CLAUDE_TOOL_ARG_FILE_PATH:-}"
if [[ "$CHANGED_FILE" == *.svelte ]] || [[ "$CHANGED_FILE" == *.ts ]]; then
    cd "$(dirname "$0")/../.." 2>/dev/null || exit 0
    if command -v prettier &>/dev/null; then
        prettier --check "$CHANGED_FILE" 2>/dev/null
        if [ $? -ne 0 ]; then
            echo "⚠ Formatting issues in $CHANGED_FILE. Run: pnpm format"
        fi
    fi
fi
