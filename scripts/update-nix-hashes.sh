#!/usr/bin/env bash
# Updates flake.nix hashes for a new release
#
# Usage: ./scripts/update-nix-hashes.sh <VERSION>
# Example: ./scripts/update-nix-hashes.sh 0.6.8
#
# Requirements: Nix package manager
#
# To install Nix (if not already installed):
#   curl --proto '=https' --tlsv1.2 -sSf -L https://install.determinate.systems/nix | sh -s -- install
#
# After running this script, test with:
#   nix build && ./result/bin/dtop --version

set -euo pipefail

VERSION="${1:-}"

if [[ -z "$VERSION" ]]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 0.6.7"
    exit 1
fi

PLATFORMS=(
    "x86_64-linux:x86_64-unknown-linux-gnu"
    "aarch64-linux:aarch64-unknown-linux-gnu"
    "x86_64-darwin:x86_64-apple-darwin"
    "aarch64-darwin:aarch64-apple-darwin"
)

echo "Updating flake.nix for version $VERSION..."

# Update version
sed -i "s/version = \"[^\"]*\"/version = \"$VERSION\"/" flake.nix

# Update hashes
for entry in "${PLATFORMS[@]}"; do
    NIX_PLATFORM="${entry%%:*}"
    RELEASE_PLATFORM="${entry##*:}"

    URL="https://github.com/amir20/dtop/releases/download/v${VERSION}/dtop-${RELEASE_PLATFORM}.tar.gz"
    echo "Fetching hash for $NIX_PLATFORM..."

    OLD_HASH=$(nix-prefetch-url "$URL" 2>/dev/null)
    SRI_HASH=$(nix hash convert --hash-algo sha256 --to sri "$OLD_HASH")

    # Update the hash in flake.nix
    sed -i "s|\"$NIX_PLATFORM\" = \"sha256-[^\"]*\"|\"$NIX_PLATFORM\" = \"$SRI_HASH\"|" flake.nix

    echo "  $NIX_PLATFORM: $SRI_HASH"
done

echo ""
echo "Done! Updated flake.nix to version $VERSION"
echo "Test with: nix build && ./result/bin/dtop --version"
