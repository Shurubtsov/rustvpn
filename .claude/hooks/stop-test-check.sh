#!/bin/bash
# Stop hook: run tests before stopping
cd "$(dirname "$0")/../../src-tauri" 2>/dev/null || exit 0
echo "Running cargo test before stopping..."
cargo test 2>&1
if [ $? -ne 0 ]; then
    echo "⚠ Some Rust tests are failing!"
fi
