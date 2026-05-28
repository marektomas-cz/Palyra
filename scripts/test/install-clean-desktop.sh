#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/test/install-clean-desktop.sh [options]

Options:
  --workspace-root <path>  Override the clean desktop harness root.
  --skip-build            Reuse existing release binaries.
  --launch                Launch the installed desktop app after install.
  --no-launch             Install only; do not launch the desktop app.
  -h, --help              Show this help.

This shell entrypoint uses the repository PowerShell release helpers as the
packaging/install source of truth, so PowerShell 7+ (`pwsh`) must be available.
USAGE
}

require_value() {
  local flag="$1"
  local value="${2:-}"
  if [[ -z "$value" ]]; then
    echo "$flag requires a value." >&2
    exit 2
  fi
}

for arg in "$@"; do
  case "$arg" in
    -h|--help)
      usage
      exit 0
      ;;
  esac
done

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
pwsh_args=(
  -NoLogo
  -File "$repo_root/scripts/test/install-clean-desktop.ps1"
)

while [[ $# -gt 0 ]]; do
  case "$1" in
    --workspace-root)
      require_value "$1" "${2:-}"
      pwsh_args+=(-WorkspaceRoot "$2")
      shift 2
      ;;
    --workspace-root=*)
      pwsh_args+=(-WorkspaceRoot "${1#*=}")
      shift
      ;;
    --skip-build)
      pwsh_args+=(-SkipBuild)
      shift
      ;;
    --launch)
      pwsh_args+=(-Launch)
      shift
      ;;
    --no-launch)
      pwsh_args+=(-NoLaunch)
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    --)
      shift
      pwsh_args+=("$@")
      break
      ;;
    *)
      pwsh_args+=("$1")
      shift
      ;;
  esac
done

if ! command -v pwsh >/dev/null 2>&1; then
  echo "pwsh is required for clean desktop install testing." >&2
  exit 1
fi

exec pwsh "${pwsh_args[@]}"
