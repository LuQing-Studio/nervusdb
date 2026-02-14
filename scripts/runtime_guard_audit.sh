#!/usr/bin/env bash
set -euo pipefail

ROOT="${1:-nervusdb-query/src/executor}"

if ! command -v rg >/dev/null 2>&1; then
  echo "[runtime-guard-audit] error: rg not found in PATH" >&2
  exit 1
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

# Output format: "<file>\t<count>\n"
rg -n "${EVAL_PAT}" "${ROOT}" -g'*.rs' \
  | cut -d: -f1 \
  | sort \
  | uniq -c \
  | awk '{print $2 "\t" $1}' \
  >"${eval_counts_file}" \
  || true

rg -n "${GUARD_PAT}" "${ROOT}" -g'*.rs' \
  | cut -d: -f1 \
  | sort \
  | uniq -c \
  | awk '{print $2 "\t" $1}' \
  >"${guard_counts_file}" \
  || true

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

echo
echo "[runtime-guard-audit] done"
