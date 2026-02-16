#!/bin/bash
set -euo pipefail

# NervusDB Rust 核心能力边界测试运行脚本
#
# 此测试需要在 nervusdb workspace 中运行。
# 确保 nervusdb-rust-test 已添加到 workspace Cargo.toml 的 members 中。

WORKSPACE_DIR="/Volumes/WorkDrive/Code/github.com/LuQing-Studio/rust/nervusdb"
cd "$WORKSPACE_DIR"

# 确保 workspace member 存在
if [ ! -d "nervusdb-rust-test" ]; then
    echo "Error: nervusdb-rust-test not found in workspace. Copy it first:"
    echo "  cp -r $(dirname "$0") $WORKSPACE_DIR/nervusdb-rust-test"
    exit 1
fi

cargo test -p nervusdb-rust-test --test test_capabilities -- --test-threads=1 --nocapture 2>&1
