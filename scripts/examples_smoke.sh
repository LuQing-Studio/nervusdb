#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[examples-smoke] build node addon"
cargo build --manifest-path nervusdb-node/Cargo.toml --release

echo "[examples-smoke] run ts-local"
npm --prefix examples/ts-local ci
npm --prefix examples/ts-local run smoke

echo "[examples-smoke] run py-local"
venv_dir="$(mktemp -d)"
cleanup() {
  rm -rf "$venv_dir"
}
trap cleanup EXIT

python3 -m venv "$venv_dir"
source "$venv_dir/bin/activate"
python -m pip install --quiet --upgrade pip
python -m pip install --quiet maturin
maturin develop -m nervusdb-pyo3/Cargo.toml
python examples/py-local/smoke.py

echo "[examples-smoke] done"
