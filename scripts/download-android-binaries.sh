#!/usr/bin/env bash
set -euo pipefail

# Download/build Android ARM64 binaries for RustVPN
# Usage: ./scripts/download-android-binaries.sh
#
# Requires: NDK_HOME or ANDROID_NDK_HOME to be set (for hev-socks5-tunnel compilation)

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
TARGET_DIR="$PROJECT_DIR/src-tauri/tauri-plugin-vpn/android/src/main/jniLibs/arm64-v8a"

XRAY_VERSION="${XRAY_VERSION:-1.8.24}"
HEV_VERSION="${HEV_VERSION:-2.14.4}"

echo "=== Preparing Android ARM64 binaries ==="
echo "xray-core version: $XRAY_VERSION"
echo "hev-socks5-tunnel version: $HEV_VERSION"
echo "Target directory: $TARGET_DIR"
echo

mkdir -p "$TARGET_DIR"

# Download xray-core (pre-built Android ARM64 binary from official releases)
echo "--- Downloading xray-core ---"
XRAY_URL="https://github.com/XTLS/Xray-core/releases/download/v${XRAY_VERSION}/Xray-android-arm64-v8a.zip"
XRAY_TMP=$(mktemp -d)
curl -fSL "$XRAY_URL" -o "$XRAY_TMP/xray.zip"
unzip -o "$XRAY_TMP/xray.zip" xray -d "$XRAY_TMP"
mv "$XRAY_TMP/xray" "$TARGET_DIR/libxray.so"
chmod +x "$TARGET_DIR/libxray.so"
rm -rf "$XRAY_TMP"
echo "xray-core downloaded and renamed to libxray.so"

# Build hev-socks5-tunnel from source using NDK
# The pre-built GitHub release is a Linux/glibc binary that cannot run on Android (bionic libc).
# We must compile from source to get a proper Android shared library.
echo "--- Building hev-socks5-tunnel from source ---"

# Find NDK
NDK="${NDK_HOME:-${ANDROID_NDK_HOME:-}}"
if [ -z "$NDK" ]; then
    # Try common locations
    if [ -n "${ANDROID_HOME:-}" ] && [ -d "$ANDROID_HOME/ndk" ]; then
        NDK=$(ls -d "$ANDROID_HOME/ndk"/*/ 2>/dev/null | sort -V | tail -1 | sed 's:/$::')
    fi
fi
if [ -z "$NDK" ] || [ ! -d "$NDK" ]; then
    echo "ERROR: NDK not found. Set NDK_HOME, ANDROID_NDK_HOME, or ANDROID_HOME."
    exit 1
fi
echo "Using NDK: $NDK"

HEV_TMP=$(mktemp -d)
git clone --depth 1 --branch "$HEV_VERSION" \
    https://github.com/heiher/hev-socks5-tunnel.git "$HEV_TMP/hev-socks5-tunnel"

cd "$HEV_TMP/hev-socks5-tunnel"
git submodule update --init --recursive

# Build as shared library using the project's Makefile with NDK cross-compilation
# hev-socks5-tunnel supports building as a shared library with ENABLE_SHARED=1
make \
    CC="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android24-clang" \
    STRIP="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-strip" \
    -j"$(nproc)" \
    shared

if [ ! -f bin/libhev-socks5-tunnel.so ]; then
    echo "ERROR: shared library build failed — bin/libhev-socks5-tunnel.so not found"
    exit 1
fi

cp bin/libhev-socks5-tunnel.so "$TARGET_DIR/libhev.so"
chmod +x "$TARGET_DIR/libhev.so"

cd "$PROJECT_DIR"
rm -rf "$HEV_TMP"
echo "hev-socks5-tunnel built from source and copied to libhev.so"

echo
echo "=== Done! Binaries placed in $TARGET_DIR ==="
ls -la "$TARGET_DIR"
