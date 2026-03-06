#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "${repo_root}"

prune_dirs=(
  "./.git"
  "./target"
  "./apps/web/node_modules"
  "./apps/web/dist"
  "./apps/web/coverage"
  "./security-artifacts"
)

allowed_paths=(
)

mapfile -t candidates < <(
  find . \
    \( \
      -path "./.git" \
      -o -path "./target" \
      -o -path "./apps/web/node_modules" \
      -o -path "./apps/web/dist" \
      -o -path "./apps/web/coverage" \
      -o -path "./security-artifacts" \
    \) -prune -o \
    -type f \
    \( \
      -iname "*.sqlite" \
      -o -iname "*.sqlite3" \
      -o -iname "*.sqlite3-*" \
      -o -iname "*.db" \
      -o -iname "*.db-*" \
      -o -iname "*.wal" \
      -o -iname "*.shm" \
      -o -iname "*.log" \
      -o -iname "support-bundle*.json" \
      -o -path "*/browser-profile/*" \
      -o -path "*/browser-profiles/*" \
      -o -path "*/downloads/*" \
    \) -print | sed 's#^\./##'
)

matches=()
for candidate in "${candidates[@]}"; do
  [[ -z "${candidate}" ]] && continue

  allowlisted=0
  for allowed_path in "${allowed_paths[@]}"; do
    if [[ "${candidate}" == "${allowed_path}" ]]; then
      allowlisted=1
      break
    fi
  done

  if [[ "${allowlisted}" -eq 0 ]]; then
    matches+=("${candidate}")
  fi
done

if [[ "${#matches[@]}" -gt 0 ]]; then
  echo "Runtime/package artifacts detected in the working tree. Remove them or move them under an explicit fixture allowlist before packaging/handoff:" >&2
  printf ' - %s\n' "${matches[@]}" >&2
  exit 1
fi

echo "Runtime artifact hygiene guard passed."
