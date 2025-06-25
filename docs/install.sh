#!/bin/bash

set -e

VERSION=${VERSION:-"latest"}
INSTALL_DIR=${INSTALL_DIR:-"/usr/local/bin"}

REPO="amir20/dtop"

# Determine latest version
if [ "$VERSION" = "latest" ]; then
  VERSION=$(curl -s "https://api.github.com/repos/$REPO/releases/latest" | grep tag_name | cut -d '"' -f 4)
fi

OS=$(uname)
ARCH=$(uname -m)

case "$ARCH" in
  x86_64) ARCH="amd64" ;;
  aarch64 | arm64) ARCH="arm64" ;;
  *) echo "⚠️ Unsupported architecture: $ARCH"; exit 1 ;;
esac

TARBALL="${REPO##*/}_${OS}_${ARCH}.tar.gz"
URL="https://github.com/$REPO/releases/download/$VERSION/$TARBALL"

echo "📥 Downloading $URL..."
curl -L "$URL" | tar -xz

echo "🔧 Installing to $INSTALL_DIR..."
sudo mv "${REPO##*/}" "$INSTALL_DIR"

echo "✅ Installed ${REPO##*/} $VERSION"
