#!/usr/bin/env bash
set -euo pipefail

ROOT="nervusdb-query/src/executor"
FAIL_ON_HOTSPOT=0

usage() {
  cat <<'USAGE'
Usage:
  scripts/runtime_guard_audit.sh [--root <dir>] [--fail-on-hotspot]

Notes:
  - Default root: nervusdb-query/src/executor
  - If --fail-on-hotspot is set, exits non-zero when any file matches:
      eval>0 && guard==0
USAGE
}

while [ $# -gt 0 ]; do
  case "$1" in
    --root)
      shift
      ROOT="${1:-}"
      if [ -z "$ROOT" ]; then
        echo "[runtime-guard-audit] error: --root requires a value" >&2
        exit 2
      fi
      ;;
    --fail-on-hotspot)
      FAIL_ON_HOTSPOT=1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      # Backwards compatible: first positional arg acts like root.
      ROOT="$1"
      ;;
  esac
  shift
done

if [ ! -d "$ROOT" ]; then
  echo "[runtime-guard-audit] error: root does not exist: $ROOT" >&2
  exit 2
fi

EVAL_PAT='evaluate_expression_value\('
GUARD_PAT='ensure_runtime_expression_compatible\('

echo "[runtime-guard-audit] root=${ROOT}"
echo "[runtime-guard-audit] eval_pat=${EVAL_PAT}"
echo "[runtime-guard-audit] guard_pat=${GUARD_PAT}"
echo

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

eval_counts_file="${tmp_dir}/eval_counts.tsv"
guard_counts_file="${tmp_dir}/guard_counts.tsv"

list_matches() {
  local pat="$1"
  local root="$2"

  if command -v rg >/dev/null 2>&1; then
    rg -n "${pat}" "${root}" -g'*.rs'
    return 0
  fi

  # Fallback for environments without ripgrep.
  grep -REn --include='*.rs' -e "${pat}" "${root}" || true
}

matches_to_counts() {
  # stdin: "<file>:<line>:..." lines
  cut -d: -f1 \
    | sort \
    | uniq -c \
    | awk '{print $2 "\t" $1}'
}

list_matches "${EVAL_PAT}" "${ROOT}" | matches_to_counts >"${eval_counts_file}" || true
list_matches "${GUARD_PAT}" "${ROOT}" | matches_to_counts >"${guard_counts_file}" || true

printf "%-78s %8s %8s\n" "file" "eval" "guard"
printf "%-78s %8s %8s\n" "----" "----" "-----"

awk -F'\t' '
  FNR==NR {
    eval[$1] = $2
    files[$1] = 1
    next
  }
  {
    guard[$1] = $2
    files[$1] = 1
  }
  END {
    for (f in files) {
      e = (f in eval) ? eval[f] : 0
      g = (f in guard) ? guard[f] : 0
      printf "%-78s %8d %8d\n", f, e, g
    }
  }
' "${eval_counts_file}" "${guard_counts_file}" | sort

echo
echo "[runtime-guard-audit] potential hotspots (eval>0 && guard==0)"
hotspots="$(
awk -F'\t' '
  FNR==NR {
    eval[$1] = $2
    files[$1] = 1
    next
  }
  {
    guard[$1] = $2
    files[$1] = 1
  }
  END {
    found = 0
    for (f in files) {
      e = (f in eval) ? eval[f] : 0
      g = (f in guard) ? guard[f] : 0
      if (e > 0 && g == 0) {
        printf "  - %s\n", f
        found = 1
      }
    }
    if (!found) {
      print "  (none)"
    }
  }
' "${eval_counts_file}" "${guard_counts_file}" | sort
)"

echo "${hotspots}"

hotspot_count="$(
  printf "%s\n" "${hotspots}" | awk '
    /^  - / {
      c++
    }
    END {
      print c + 0
    }
  '
)"

if [ "$FAIL_ON_HOTSPOT" -eq 1 ] && [ "${hotspot_count}" -gt 0 ]; then
  echo "[runtime-guard-audit] BLOCKED: hotspots detected" >&2
  exit 1
fi

echo
echo "[runtime-guard-audit] done"
