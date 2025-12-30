#!/bin/bash
set -e

# NervusDB Release Script
# Usage: ./release.sh [version]

VERSION=${1:-$(git describe --tags --always)}
echo "Building nervusdb-cli v${VERSION}"

# Create release directory
RELEASE_DIR="release-${VERSION}"
mkdir -p "${RELEASE_DIR}"

# Build for different platforms
echo "Building for x86_64-unknown-linux-gnu..."
cargo build --release --package nervusdb-cli
cp target/release/nervusdb "${RELEASE_DIR}/nervusdb-linux-x64"
gzip -f "${RELEASE_DIR}/nervusdb-linux-x64"

echo "Building for x86_64-apple-darwin..."
cargo build --release --package nervusdb-cli --target x86_64-apple-darwin
cp target/x86_64-apple-darwin/release/nervusdb "${RELEASE_DIR}/nervusdb-macos-x64"
gzip -f "${RELEASE_DIR}/nervusdb-macos-x64"

echo "Building for aarch64-apple-darwin..."
cargo build --release --package nervusdb-cli --target aarch64-apple-darwin
cp target/aarch64-apple-darwin/release/nervusdb "${RELEASE_DIR}/nervusdb-macos-arm64"
gzip -f "${RELEASE_DIR}/nervusdb-macos-arm64"

echo "Building for x86_64-pc-windows-msvc..."
cargo build --release --package nervusdb-cli --target x86_64-pc-windows-msvc
cp target/x86_64-pc-windows-msvc/release/nervusdb.exe "${RELEASE_DIR}/nervusdb-windows-x64.exe"
gzip -f "${RELEASE_DIR}/nervusdb-windows-x64.exe"

echo ""
echo "Release files created in ${RELEASE_DIR}/"
ls -la "${RELEASE_DIR}/"

echo ""
echo "To create GitHub Release:"
echo "gh release create v${VERSION} ${RELEASE_DIR}/*.gz --title 'NervusDB v${VERSION}'"
