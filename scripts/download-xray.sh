#!/bin/bash
# Downloads the xray-core binary for the current platform.
# Usage: ./scripts/download-xray.sh [version]
#
# The binary is placed in src-tauri/binaries/ with the Tauri sidecar
# naming convention: xray-<target-triple>[.exe]

set -euo pipefail

XRAY_VERSION="${1:-v26.2.6}"
BINARIES_DIR="$(cd "$(dirname "$0")/../src-tauri/binaries" && pwd)"
BASE_URL="https://github.com/XTLS/Xray-core/releases/download/${XRAY_VERSION}"

# Detect OS
case "$(uname -s)" in
    Linux)   OS="linux" ;;
    Darwin)  OS="darwin" ;;
    MINGW*|MSYS*|CYGWIN*|Windows_NT)
             OS="windows" ;;
    *)
        echo "Error: Unsupported OS: $(uname -s)"
        exit 1
        ;;
esac

# Detect architecture
case "$(uname -m)" in
    x86_64|amd64)  ARCH="x86_64" ;;
    aarch64|arm64) ARCH="aarch64" ;;
    *)
        echo "Error: Unsupported architecture: $(uname -m)"
        exit 1
        ;;
esac

# Map to xray release archive name and Tauri sidecar target triple
case "${OS}-${ARCH}" in
    linux-x86_64)
        ARCHIVE="Xray-linux-64.zip"
        TARGET_TRIPLE="x86_64-unknown-linux-gnu"
        BINARY_NAME="xray-${TARGET_TRIPLE}"
        ;;
    linux-aarch64)
        ARCHIVE="Xray-linux-arm64-v8a.zip"
        TARGET_TRIPLE="aarch64-unknown-linux-gnu"
        BINARY_NAME="xray-${TARGET_TRIPLE}"
        ;;
    darwin-aarch64)
        ARCHIVE="Xray-macos-arm64-v8a.zip"
        TARGET_TRIPLE="aarch64-apple-darwin"
        BINARY_NAME="xray-${TARGET_TRIPLE}"
        ;;
    darwin-x86_64)
        ARCHIVE="Xray-macos-64.zip"
        TARGET_TRIPLE="x86_64-apple-darwin"
        BINARY_NAME="xray-${TARGET_TRIPLE}"
        ;;
    windows-x86_64)
        ARCHIVE="Xray-windows-64.zip"
        TARGET_TRIPLE="x86_64-pc-windows-msvc"
        BINARY_NAME="xray-${TARGET_TRIPLE}.exe"
        ;;
    *)
        echo "Error: No xray binary available for ${OS}-${ARCH}"
        exit 1
        ;;
esac

DOWNLOAD_URL="${BASE_URL}/${ARCHIVE}"
TEMP_DIR="$(mktemp -d)"

cleanup() {
    rm -rf "${TEMP_DIR}"
}
trap cleanup EXIT

echo "Downloading xray-core ${XRAY_VERSION} for ${OS}/${ARCH}..."
echo "  URL: ${DOWNLOAD_URL}"

curl -sL "${DOWNLOAD_URL}" -o "${TEMP_DIR}/xray.zip"

echo "Extracting..."
if [ "${OS}" = "windows" ]; then
    unzip -o "${TEMP_DIR}/xray.zip" "xray.exe" -d "${TEMP_DIR}/extracted"
    SOURCE_BIN="${TEMP_DIR}/extracted/xray.exe"
else
    unzip -o "${TEMP_DIR}/xray.zip" "xray" -d "${TEMP_DIR}/extracted"
    SOURCE_BIN="${TEMP_DIR}/extracted/xray"
fi

mkdir -p "${BINARIES_DIR}"
mv "${SOURCE_BIN}" "${BINARIES_DIR}/${BINARY_NAME}"

# Make executable (non-Windows)
if [ "${OS}" != "windows" ]; then
    chmod +x "${BINARIES_DIR}/${BINARY_NAME}"
fi

echo "Done! Binary saved to: ${BINARIES_DIR}/${BINARY_NAME}"
echo ""
echo "To download binaries for all platforms (cross-compilation), run:"
echo "  ./scripts/download-xray-all.sh"
