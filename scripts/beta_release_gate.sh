#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

REPORT_DIR="${TCK_REPORT_DIR:-artifacts/tck}"
WINDOW_FILE="${STABILITY_WINDOW_FILE:-${REPORT_DIR}/stability-window.json}"
REQUIRED_DAYS="${STABILITY_DAYS:-7}"
MODE="strict"
GITHUB_REPO="${STABILITY_GITHUB_REPO:-${GITHUB_REPOSITORY:-}}"
GITHUB_TOKEN_ENV="${STABILITY_GITHUB_TOKEN_ENV:-GITHUB_TOKEN}"

usage() {
  cat <<'USAGE'
Usage:
  scripts/beta_release_gate.sh [options]

Options:
  --window-file FILE             Stability window JSON (default: artifacts/tck/stability-window.json)
  --required-days N              Required consecutive days (default: STABILITY_DAYS or 7)
  --mode strict|tier3-only       Forwarded to stability_window.sh when rebuilding (default: strict)
  --github-repo owner/repo       Forwarded repo hint for stability_window.sh
  --github-token-env ENV_NAME    Forwarded token env for stability_window.sh (default: GITHUB_TOKEN)
  -h, --help                     Show this help
USAGE
}

while [ $# -gt 0 ]; do
  case "$1" in
    --window-file)
      shift
      WINDOW_FILE="${1:-}"
      if [ -z "$WINDOW_FILE" ]; then
        echo "[beta-release-gate] error: --window-file requires a value" >&2
        exit 2
      fi
      ;;
    --required-days)
      shift
      REQUIRED_DAYS="${1:-}"
      if [ -z "$REQUIRED_DAYS" ]; then
        echo "[beta-release-gate] error: --required-days requires a value" >&2
        exit 2
      fi
      ;;
    --mode)
      shift
      MODE="${1:-}"
      if [ -z "$MODE" ]; then
        echo "[beta-release-gate] error: --mode requires a value" >&2
        exit 2
      fi
      ;;
    --github-repo)
      shift
      GITHUB_REPO="${1:-}"
      if [ -z "$GITHUB_REPO" ]; then
        echo "[beta-release-gate] error: --github-repo requires a value" >&2
        exit 2
      fi
      ;;
    --github-token-env)
      shift
      GITHUB_TOKEN_ENV="${1:-}"
      if [ -z "$GITHUB_TOKEN_ENV" ]; then
        echo "[beta-release-gate] error: --github-token-env requires a value" >&2
        exit 2
      fi
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "[beta-release-gate] error: unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

if ! [[ "$REQUIRED_DAYS" =~ ^[0-9]+$ ]] || [ "$REQUIRED_DAYS" -le 0 ]; then
  echo "[beta-release-gate] invalid --required-days: $REQUIRED_DAYS" >&2
  exit 2
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "[beta-release-gate] error: jq not found in PATH" >&2
  exit 2
fi

mkdir -p "$REPORT_DIR"

if [ ! -f "$WINDOW_FILE" ]; then
  echo "[beta-release-gate] no stability window report found, rebuilding..."
  bash scripts/stability_window.sh \
    --mode "$MODE" \
    --github-repo "$GITHUB_REPO" \
    --github-token-env "$GITHUB_TOKEN_ENV" || true
fi

if [ ! -f "$WINDOW_FILE" ]; then
  echo "[beta-release-gate] BLOCKED: missing stability window report: $WINDOW_FILE" >&2
  exit 1
fi

consecutive_days="$(jq -r '.consecutive_days // 0' "$WINDOW_FILE")"
all_checks_pass="$(jq -r '.all_checks_pass // false' "$WINDOW_FILE")"
window_passed="$(jq -r '.window_passed // false' "$WINDOW_FILE")"
as_of_date="$(jq -r '.as_of_date // ""' "$WINDOW_FILE")"

if ! [[ "$consecutive_days" =~ ^[0-9]+$ ]]; then
  echo "[beta-release-gate] BLOCKED: invalid consecutive_days in $WINDOW_FILE" >&2
  exit 1
fi

echo "[beta-release-gate] as_of_date=${as_of_date}"
echo "[beta-release-gate] consecutive_days=${consecutive_days}/${REQUIRED_DAYS}"
echo "[beta-release-gate] all_checks_pass=${all_checks_pass}"
echo "[beta-release-gate] window_passed=${window_passed}"

if [ "$consecutive_days" -lt "$REQUIRED_DAYS" ]; then
  echo "[beta-release-gate] BLOCKED: consecutive days below required threshold" >&2
  exit 1
fi

if [ "$all_checks_pass" != "true" ] || [ "$window_passed" != "true" ]; then
  echo "[beta-release-gate] BLOCKED: stability checks not fully passed" >&2
  exit 1
fi

echo "[beta-release-gate] PASSED"
