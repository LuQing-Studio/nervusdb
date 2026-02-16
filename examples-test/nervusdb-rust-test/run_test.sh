#!/bin/bash
set -euo pipefail

# NervusDB Rust 核心能力边界测试运行脚本（独立于 workspace members）

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

cargo test \
  --manifest-path "$SCRIPT_DIR/Cargo.toml" \
  --test test_capabilities \
  -- \
  --test-threads=1 \
  --nocapture
