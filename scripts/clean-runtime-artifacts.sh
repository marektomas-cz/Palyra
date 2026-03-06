#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "${repo_root}"

state_roots=(
  "./apps/desktop/src-tauri/data"
  "./crates/palyra-cli/data"
  "./data"
)

for dir in "${state_roots[@]}"; do
  [[ -d "${dir}" ]] || continue
  find "${dir}" -type f \
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
done

find . -maxdepth 1 -type f -iname "support-bundle*.json" -delete

echo "Removed known runtime artifacts from repo-local state roots."
