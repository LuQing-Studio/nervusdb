# Python Local Example

Minimal local embedding example for `nervusdb-pyo3`.

## Run

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install --upgrade pip
pip install maturin
maturin develop -m nervusdb-pyo3/Cargo.toml
python examples/py-local/smoke.py
```

The example validates `open -> execute_write -> query_stream -> close`.
