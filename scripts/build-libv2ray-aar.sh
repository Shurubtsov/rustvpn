#!/usr/bin/env bash
set -euo pipefail

# Build AndroidLibXrayLite AAR for RustVPN
# Usage: ./scripts/build-libv2ray-aar.sh
#
# Produces libv2ray.aar via gomobile bind, copies to plugin libs dir.
# Requires: Go 1.22+, Android SDK (ANDROID_HOME set)

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
LIBS_DIR="$PROJECT_DIR/src-tauri/tauri-plugin-vpn/android/libs"

echo "=== Building AndroidLibXrayLite AAR ==="

# Verify Go is available
if ! command -v go &>/dev/null; then
    echo "ERROR: Go not found. Install Go 1.22+ and ensure it is on PATH."
    exit 1
fi
echo "Go version: $(go version)"

# Verify Android SDK
if [ -z "${ANDROID_HOME:-}" ]; then
    echo "ERROR: ANDROID_HOME not set."
    exit 1
fi
echo "ANDROID_HOME: $ANDROID_HOME"

# Install gomobile if not present
if ! command -v gomobile &>/dev/null; then
    echo "Installing gomobile..."
    go install golang.org/x/mobile/cmd/gomobile@latest
    go install golang.org/x/mobile/cmd/gobind@latest
fi
echo "gomobile: $(which gomobile)"

# Initialize gomobile (downloads NDK toolchain bindings)
gomobile init

BUILD_TMP=$(mktemp -d)
trap 'rm -rf "$BUILD_TMP"' EXIT

echo "--- Cloning AndroidLibXrayLite ---"
git clone --depth 1 https://github.com/2dust/AndroidLibXrayLite.git "$BUILD_TMP/AndroidLibXrayLite"

cd "$BUILD_TMP/AndroidLibXrayLite"

echo "--- Running gomobile bind ---"
gomobile bind -v \
    -target=android/arm64 \
    -androidapi 21 \
    -ldflags='-s -w' \
    -o "$BUILD_TMP/libv2ray.aar" \
    ./

echo "--- Copying AAR to plugin libs ---"
mkdir -p "$LIBS_DIR"
cp "$BUILD_TMP/libv2ray.aar" "$LIBS_DIR/libv2ray.aar"

cd "$PROJECT_DIR"
echo
echo "=== Done! AAR placed at $LIBS_DIR/libv2ray.aar ==="
ls -la "$LIBS_DIR/libv2ray.aar"
