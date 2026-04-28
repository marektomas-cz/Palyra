#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "${repo_root}"

recursive_state_roots=(
  "./apps/desktop/src-tauri/data"
  "./crates/palyra-cli/data"
)

shallow_state_roots=(
  "./data"
)

delete_runtime_files() {
  local dir="$1"
  shift
  [[ -d "${dir}" ]] || return 0

  find "${dir}" "$@" -type f \
    \( \
      -iname "*.sqlite" \
      -o -iname "*.sqlite3" \
      -o -iname "*.sqlite3-*" \
      -o -iname "*.db" \
      -o -iname "*.db-*" \
      -o -iname "*.wal" \
      -o -iname "*.shm" \
      -o -iname "*.log" \
    \) -delete
}

for dir in "${recursive_state_roots[@]}"; do
  delete_runtime_files "${dir}"
done

for dir in "${shallow_state_roots[@]}"; do
  [[ -d "${dir}" ]] || continue
  delete_runtime_files "${dir}" -maxdepth 1
done

find . -maxdepth 1 -type f -iname "support-bundle*.json" -delete

echo "Removed known runtime artifacts from repo-local state roots."
