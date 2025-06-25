#!/bin/bash

set -e

VERSION=${VERSION:-"latest"}
INSTALL_DIR=${INSTALL_DIR:-"/usr/local/bin"}

REPO="amir20/dtop"
BINARY_NAME="dtop"

# Determine latest version
if [ "$VERSION" = "latest" ]; then
  VERSION=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep tag_name | cut -d '"' -f 4)
fi

OS=$(uname)
ARCH=$(uname -m)

# Convert OS to lowercase and handle different naming
case "$OS" in
  Linux) OS="linux" ;;
  Darwin) OS="darwin" ;;
  FreeBSD) OS="freebsd" ;;
  MINGW* | MSYS* | CYGWIN*) OS="windows" ;;
  *) echo "⚠️ Unsupported OS: $OS"; exit 1 ;;
esac

# Convert architecture to match goreleaser naming
case "$ARCH" in
  x86_64)
    if [ "$OS" = "darwin" ] || [ "$OS" = "linux" ] || [ "$OS" = "freebsd" ] || [ "$OS" = "windows" ]; then
      ARCH="amd64_v1"
    else
      ARCH="amd64"
    fi
    ;;
  aarch64 | arm64) ARCH="arm64_v8.0" ;;
  armv6l) ARCH="arm_6" ;;
  i386 | i686) ARCH="386_sse2" ;;
  *) echo "⚠️ Unsupported architecture: $ARCH"; exit 1 ;;
esac

# Determine binary extension
BINARY_EXT=""
if [ "$OS" = "windows" ]; then
  BINARY_EXT=".exe"
fi

# Construct the tarball name based on your goreleaser pattern
TARBALL="${BINARY_NAME}_${OS}_${ARCH}.tar.gz"
URL="https://github.com/$REPO/releases/download/$VERSION/$TARBALL"

echo "📥 Downloading $URL..."

# Create temporary directory
TEMP_DIR=$(mktemp -d)
cd "$TEMP_DIR"

# Download and extract
curl -L "$URL" | tar -xz

# Find the binary (it should be named termui or termui.exe)
BINARY_PATH="${BINARY_NAME}${BINARY_EXT}"

if [ ! -f "$BINARY_PATH" ]; then
  echo "⚠️ Binary $BINARY_PATH not found in archive"
  ls -la
  exit 1
fi

echo "🔧 Installing to $INSTALL_DIR..."
sudo mv "$BINARY_PATH" "$INSTALL_DIR/$BINARY_NAME"

# Cleanup
rm -rf "$TEMP_DIR"

echo "✅ Installed $BINARY_NAME $VERSION to $INSTALL_DIR"
