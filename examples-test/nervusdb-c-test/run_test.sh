#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEST_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

cargo build -p nervusdb-capi --release

CC_BIN="${CC:-cc}"
BIN_PATH="${TEST_DIR}/c-binding-smoke"

"${CC_BIN}" \
  "${TEST_DIR}/test_capabilities.c" \
  -I"${ROOT_DIR}/nervusdb-c-sdk/include" \
  -L"${ROOT_DIR}/target/release" \
  -lnervusdb \
  -o "${BIN_PATH}"

if [[ "$(uname)" == "Darwin" ]]; then
  DYLD_LIBRARY_PATH="${ROOT_DIR}/target/release:${DYLD_LIBRARY_PATH:-}" "${BIN_PATH}"
else
  LD_LIBRARY_PATH="${ROOT_DIR}/target/release:${LD_LIBRARY_PATH:-}" "${BIN_PATH}"
fi
