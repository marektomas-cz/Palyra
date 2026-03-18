#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "${repo_root}"

warn_threshold="${PALYRA_MODULE_BUDGET_WARN:-800}"
critical_threshold="${PALYRA_MODULE_BUDGET_CRITICAL:-1200}"
entrypoint_threshold="${PALYRA_MODULE_BUDGET_ENTRYPOINT:-200}"

if ! command -v git >/dev/null 2>&1; then
  echo "git is required to report module budgets" >&2
  exit 1
fi

include_regex='\.(rs|proto|ts|tsx|js|mjs|sh|ps1|css|html)$'
exclude_regex='^(node_modules/|schemas/generated/|apps/web/dist/|apps/web/.vite/|apps/desktop/ui/dist/|apps/desktop/ui/.vite/|target/|fuzz/target/|security-artifacts/)'

declare -a warn_files=()
declare -a critical_files=()
declare -a large_entrypoints=()
declare -A discord_counts=(
  ["apps"]=0
  ["crates/palyra-daemon"]=0
  ["crates/palyra-connectors"]=0
  ["crates/palyra-cli"]=0
  ["docs"]=0
)

count_keyword_hits() {
  local path="$1"
  (git grep -I -i -o 'discord' -- "$path" 2>/dev/null || true) | wc -l
}

while IFS= read -r path; do
  [[ -z "$path" ]] && continue
  [[ "$path" =~ $exclude_regex ]] && continue
  [[ ! "$path" =~ $include_regex ]] && continue
  [[ ! -f "$path" ]] && continue

  line_count="$(wc -l < "$path")"

  if (( line_count >= critical_threshold )); then
    critical_files+=("$(printf '%8d %s' "$line_count" "$path")")
  elif (( line_count >= warn_threshold )); then
    warn_files+=("$(printf '%8d %s' "$line_count" "$path")")
  fi

  case "$path" in
    */main.rs|*/lib.rs)
      if (( line_count >= entrypoint_threshold )); then
        large_entrypoints+=("$(printf '%8d %s' "$line_count" "$path")")
      fi
      ;;
  esac
done < <(git ls-files)

for scope in "${!discord_counts[@]}"; do
  if [[ -d "$scope" ]]; then
    discord_counts["$scope"]="$(count_keyword_hits "$scope" | tr -d '[:space:]')"
  fi
done

echo "Palyra module budget report"
echo "repo=${repo_root}"
echo "warn_threshold=${warn_threshold}"
echo "critical_threshold=${critical_threshold}"
echo "entrypoint_threshold=${entrypoint_threshold}"
echo

echo "Files at or above critical threshold (${critical_threshold}+ LOC):"
if (( ${#critical_files[@]} == 0 )); then
  echo "  none"
else
  printf '%s\n' "${critical_files[@]}" | sort -nr
fi
echo

echo "Files at or above warning threshold (${warn_threshold}+ LOC, excluding critical):"
if (( ${#warn_files[@]} == 0 )); then
  echo "  none"
else
  printf '%s\n' "${warn_files[@]}" | sort -nr
fi
echo

echo "Large root entrypoints (main.rs/lib.rs at ${entrypoint_threshold}+ LOC):"
if (( ${#large_entrypoints[@]} == 0 )); then
  echo "  none"
else
  printf '%s\n' "${large_entrypoints[@]}" | sort -nr
fi
echo

echo "Connector keyword scatter ('discord' raw hits by scope):"
for scope in apps crates/palyra-daemon crates/palyra-connectors crates/palyra-cli docs; do
  printf '  %-26s %s\n' "${scope}" "${discord_counts[$scope]}"
done
