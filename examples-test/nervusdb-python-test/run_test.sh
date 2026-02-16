#!/bin/bash
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../../rust/nervusdb" && pwd)"

# 创建临时 venv 或复用已有的
VENV_DIR="${SCRIPT_DIR}/.venv"
if [ ! -d "$VENV_DIR" ]; then
  python3 -m venv "$VENV_DIR"
fi
source "$VENV_DIR/bin/activate"
pip install --quiet --upgrade pip maturin

# 用 maturin develop 安装 nervusdb 到 venv（必须从 workspace 根目录运行）
cd "$REPO_ROOT"
maturin develop -m nervusdb-pyo3/Cargo.toml

# 运行测试
python "$SCRIPT_DIR/test_capabilities.py" "$@"
