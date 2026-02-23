#!/bin/bash
# PostToolUse hook: track edited files for session context
TRACKER_FILE="${CLAUDE_PROJECT_DIR:-.}/.claude/.edited-files"
CHANGED_FILE="${CLAUDE_TOOL_ARG_FILE_PATH:-}"
if [ -n "$CHANGED_FILE" ]; then
    echo "$CHANGED_FILE" >> "$TRACKER_FILE"
    sort -u "$TRACKER_FILE" -o "$TRACKER_FILE" 2>/dev/null
fi
