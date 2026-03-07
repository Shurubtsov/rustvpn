#!/bin/bash
# Install the RustVPN privileged helper and polkit policy.
# Run this once after installation: sudo ./scripts/install-helper.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo "Installing RustVPN helper..."

# Install helper script
install -m 755 "$PROJECT_DIR/scripts/rustvpn-helper" /usr/local/bin/rustvpn-helper

# Install polkit policy
install -m 644 "$PROJECT_DIR/polkit/com.rustvpn.vpn.policy" /usr/share/polkit-1/actions/com.rustvpn.vpn.policy

echo "Done. Password will be cached for 5 minutes after first authentication."
