#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

python3 - "$ROOT_DIR" <<'PY'
import sys
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover
    import tomli as tomllib  # type: ignore

root = Path(sys.argv[1])

targets = [
    ("nervusdb-node", root / "nervusdb-node" / "Cargo.toml"),
    ("nervusdb-pyo3", root / "nervusdb-pyo3" / "Cargo.toml"),
]

forbidden_packages = {
    "nervusdb",
    "nervusdb-api",
    "nervusdb-query",
    "nervusdb-storage",
}
required_package = "nervusdb-capi"
dep_sections = ("dependencies", "dev-dependencies", "build-dependencies")


def iter_deps(doc: dict):
    for section in dep_sections:
        yield section, doc.get(section, {})

    target_table = doc.get("target", {})
    if isinstance(target_table, dict):
        for target_name, target_cfg in target_table.items():
            if not isinstance(target_cfg, dict):
                continue
            for section in dep_sections:
                key = f"target.{target_name}.{section}"
                yield key, target_cfg.get(section, {})


violations = []
missing_required = []

for crate_name, manifest in targets:
    data = tomllib.loads(manifest.read_text())
    seen_packages = set()

    for section, deps in iter_deps(data):
        if not isinstance(deps, dict):
            continue
        for dep_key, dep_spec in deps.items():
            package_name = dep_key
            if isinstance(dep_spec, dict):
                package_name = dep_spec.get("package", dep_key)
            seen_packages.add(package_name)
            if package_name in forbidden_packages:
                violations.append(
                    f"{crate_name}: {section} -> {dep_key} (package={package_name})"
                )

    if required_package not in seen_packages:
        missing_required.append(crate_name)

if violations:
    print("[abi-binding-dep-gate] BLOCKED: forbidden direct business dependencies detected:")
    for item in violations:
        print(f"  - {item}")
    sys.exit(1)

if missing_required:
    print(
        "[abi-binding-dep-gate] BLOCKED: thin bindings must depend on nervusdb-capi, missing:"
    )
    for item in missing_required:
        print(f"  - {item}")
    sys.exit(1)

print("[abi-binding-dep-gate] PASSED")
PY
