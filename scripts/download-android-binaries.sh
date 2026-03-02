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

# Build hev-socks5-tunnel from source using ndk-build
# The pre-built GitHub release is a Linux/glibc binary that cannot run on Android (bionic libc).
# We use the project's own Android.mk which sets FD_SET_DEFINED and SOCKLEN_T_DEFINED
# to avoid typedef conflicts between lwip and Android's bionic headers.
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

# ndk-build expects the source in a 'jni' subdirectory
mkdir -p "$HEV_TMP/hev-build"
git clone --depth 1 --branch "$HEV_VERSION" --recursive \
    https://github.com/heiher/hev-socks5-tunnel.git "$HEV_TMP/hev-build/jni"

cd "$HEV_TMP/hev-build"
"$NDK/ndk-build" APP_ABI=arm64-v8a -j"$(nproc)"

# ndk-build outputs to libs/<ABI>/libhev-socks5-tunnel.so
HEV_SO="$HEV_TMP/hev-build/libs/arm64-v8a/libhev-socks5-tunnel.so"
if [ ! -f "$HEV_SO" ]; then
    echo "ERROR: ndk-build failed — $HEV_SO not found"
    ls -la "$HEV_TMP/hev-build/libs/" 2>/dev/null || true
    exit 1
fi

cp "$HEV_SO" "$TARGET_DIR/libhev.so"
chmod +x "$TARGET_DIR/libhev.so"

cd "$PROJECT_DIR"
rm -rf "$HEV_TMP"
echo "hev-socks5-tunnel built from source and copied to libhev.so"

echo
echo "=== Done! Binaries placed in $TARGET_DIR ==="
ls -la "$TARGET_DIR"
