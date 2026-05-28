#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/test/uninstall-clean-desktop.sh [options]

Options:
  --workspace-root <path>  Override the clean desktop harness root.
  --keep-artifacts         Preserve build/package artifacts.
  -h, --help               Show this help.

This shell entrypoint uses the repository PowerShell release helpers as the
install/uninstall source of truth, so PowerShell 7+ (`pwsh`) must be available.
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
  -File "$repo_root/scripts/test/uninstall-clean-desktop.ps1"
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
    --keep-artifacts)
      pwsh_args+=(-KeepArtifacts)
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
  echo "pwsh is required for clean desktop uninstall testing." >&2
  exit 1
fi

exec pwsh "${pwsh_args[@]}"
