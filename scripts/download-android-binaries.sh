#!/usr/bin/env bash
set -euo pipefail

# Download Android ARM64 binaries for RustVPN
# Usage: ./scripts/download-android-binaries.sh

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
TARGET_DIR="$PROJECT_DIR/src-tauri/tauri-plugin-vpn/android/src/main/jniLibs/arm64-v8a"

XRAY_VERSION="${XRAY_VERSION:-1.8.24}"
HEV_VERSION="${HEV_VERSION:-2.14.4}"

echo "=== Downloading Android ARM64 binaries ==="
echo "xray-core version: $XRAY_VERSION"
echo "hev-socks5-tunnel version: $HEV_VERSION"
echo "Target directory: $TARGET_DIR"
echo

mkdir -p "$TARGET_DIR"

# Download xray-core
echo "--- Downloading xray-core ---"
XRAY_URL="https://github.com/XTLS/Xray-core/releases/download/v${XRAY_VERSION}/Xray-android-arm64-v8a.zip"
XRAY_TMP=$(mktemp -d)
curl -fSL "$XRAY_URL" -o "$XRAY_TMP/xray.zip"
unzip -o "$XRAY_TMP/xray.zip" xray -d "$XRAY_TMP"
mv "$XRAY_TMP/xray" "$TARGET_DIR/libxray.so"
chmod +x "$TARGET_DIR/libxray.so"
rm -rf "$XRAY_TMP"
echo "xray-core downloaded and renamed to libxray.so"

# Download hev-socks5-tunnel
echo "--- Downloading hev-socks5-tunnel ---"
HEV_URL="https://github.com/heiher/hev-socks5-tunnel/releases/download/${HEV_VERSION}/hev-socks5-tunnel-linux-arm64"
curl -fSL "$HEV_URL" -o "$TARGET_DIR/libhev.so"
chmod +x "$TARGET_DIR/libhev.so"
echo "hev-socks5-tunnel downloaded and renamed to libhev.so"

echo
echo "=== Done! Binaries placed in $TARGET_DIR ==="
ls -la "$TARGET_DIR"
