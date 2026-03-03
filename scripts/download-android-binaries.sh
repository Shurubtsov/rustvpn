#!/usr/bin/env bash
set -euo pipefail

# Build Android ARM64 binaries for RustVPN
# Usage: ./scripts/download-android-binaries.sh
#
# Requires:
#   - NDK_HOME or ANDROID_NDK_HOME set (for hev-socks5-tunnel compilation)
#   - Go 1.22+ and ANDROID_HOME set (for AndroidLibXrayLite AAR build)

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
JNILIBS_DIR="$PROJECT_DIR/src-tauri/tauri-plugin-vpn/android/src/main/jniLibs/arm64-v8a"
LIBS_DIR="$PROJECT_DIR/src-tauri/tauri-plugin-vpn/android/libs"

HEV_VERSION="${HEV_VERSION:-2.14.4}"

echo "=== Preparing Android ARM64 binaries ==="
echo "hev-socks5-tunnel version: $HEV_VERSION"
echo "jniLibs directory: $JNILIBS_DIR"
echo "AAR libs directory: $LIBS_DIR"
echo

mkdir -p "$JNILIBS_DIR"
mkdir -p "$LIBS_DIR"

# Build AndroidLibXrayLite AAR via gomobile
echo "--- Building AndroidLibXrayLite AAR ---"

if ! command -v go &>/dev/null; then
    echo "ERROR: Go not found. Install Go 1.22+ and ensure it is on PATH."
    exit 1
fi
echo "Go version: $(go version)"

if [ -z "${ANDROID_HOME:-}" ]; then
    echo "ERROR: ANDROID_HOME not set."
    exit 1
fi

# Install gomobile if not present
if ! command -v gomobile &>/dev/null; then
    echo "Installing gomobile..."
    go install golang.org/x/mobile/cmd/gomobile@latest
    go install golang.org/x/mobile/cmd/gobind@latest
fi
gomobile init

AAR_TMP=$(mktemp -d)
git clone --depth 1 https://github.com/niclas-niclas/AndroidLibXrayLite.git "$AAR_TMP/AndroidLibXrayLite"

cd "$AAR_TMP/AndroidLibXrayLite"
gomobile bind -v \
    -target=android/arm64 \
    -androidapi 21 \
    -ldflags='-s -w' \
    -o "$AAR_TMP/libv2ray.aar" \
    ./

cp "$AAR_TMP/libv2ray.aar" "$LIBS_DIR/libv2ray.aar"
cd "$PROJECT_DIR"
rm -rf "$AAR_TMP"
echo "AndroidLibXrayLite AAR built and copied to $LIBS_DIR/libv2ray.aar"

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

cp "$HEV_SO" "$JNILIBS_DIR/libhev.so"
chmod +x "$JNILIBS_DIR/libhev.so"

cd "$PROJECT_DIR"
rm -rf "$HEV_TMP"
echo "hev-socks5-tunnel built from source and copied to libhev.so"

echo
echo "=== Done! ==="
echo "AAR: $LIBS_DIR/libv2ray.aar"
echo "jniLibs:"
ls -la "$JNILIBS_DIR"
