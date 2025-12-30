#!/bin/bash
set -e

# NervusDB Release Script (Linux only)
# Usage: ./release.sh [version]

VERSION=${1:-$(git describe --tags --always)}
echo "Building nervusdb-cli v${VERSION}"

# Create release directory
RELEASE_DIR="release-${VERSION}"
mkdir -p "${RELEASE_DIR}"

# Build for Linux x86_64 only
echo "Building for x86_64-unknown-linux-gnu..."
cargo build --release --package nervusdb-cli
cp target/release/nervusdb "${RELEASE_DIR}/nervusdb-linux-x64"
gzip -f "${RELEASE_DIR}/nervusdb-linux-x64"

echo ""
echo "Release files created in ${RELEASE_DIR}/"
ls -la "${RELEASE_DIR}/"

echo ""
echo "To create GitHub Release:"
echo "gh release create v${VERSION} ${RELEASE_DIR}/*.gz --title 'NervusDB v${VERSION}'"

echo ""
echo "Note: macOS/Windows builds require macOS/Windows CI runners"
