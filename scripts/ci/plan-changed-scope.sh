#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CONFIG_PATH="${PALYRA_CHANGED_SCOPE_MAP:-$ROOT_DIR/scripts/ci/changed-scope-map.tsv}"
MAX_CHANGED_FILES="${PALYRA_CHANGED_SCOPE_MAX_FILES:-200}"

json_quote() {
  local value="$1"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  value="${value//$'\n'/\\n}"
  value="${value//$'\r'/}"
  printf '"%s"' "$value"
}

join_json_array() {
  local item
  local first=true
  printf '['
  for item in "$@"; do
    if [[ "$first" == true ]]; then
      first=false
    else
      printf ','
    fi
    json_quote "$item"
  done
  printf ']'
}

contains_item() {
  local needle="$1"
  shift
  local item
  for item in "$@"; do
    [[ "$item" == "$needle" ]] && return 0
  done
  return 1
}

append_unique_ref() {
  local -n target_ref="$1"
  local value="$2"
  [[ -z "$value" ]] && return 0
  contains_item "$value" "${target_ref[@]}" && return 0
  target_ref+=("$value")
}

changed_files_from_env() {
  printf '%s\n' "${PALYRA_CHANGED_SCOPE_FILES:-}" |
    tr ',' '\n' |
    sed 's/^[[:space:]]*//; s/[[:space:]]*$//' |
    sed '/^$/d'
}

discover_base_ref() {
  if [[ -n "${PALYRA_CHANGED_SCOPE_BASE:-}" ]]; then
    printf '%s\n' "$PALYRA_CHANGED_SCOPE_BASE"
    return 0
  fi
  if git rev-parse --verify origin/main >/dev/null 2>&1; then
    printf 'origin/main\n'
    return 0
  fi
  if git rev-parse --verify HEAD^ >/dev/null 2>&1; then
    printf 'HEAD^\n'
    return 0
  fi
  printf '\n'
}

discover_changed_files() {
  if [[ -n "${PALYRA_CHANGED_SCOPE_FILES:-}" ]]; then
    changed_files_from_env
    return 0
  fi

  local base_ref
  base_ref="$(discover_base_ref)"
  if [[ -z "$base_ref" ]]; then
    return 0
  fi
  git diff --name-only "$base_ref" --
}

command_for_scope() {
  case "$1" in
    rust:workspace) printf '%s\n' 'cargo test --workspace --locked' ;;
    rust:palyra-daemon) printf '%s\n' 'cargo test -p palyra-daemon --locked' ;;
    rust:palyra-cli) printf '%s\n' 'cargo test -p palyra-cli --locked' ;;
    rust:palyra-policy) printf '%s\n' 'cargo test -p palyra-policy --locked' ;;
    rust:palyra-connectors) printf '%s\n' 'cargo test -p palyra-connectors --locked' ;;
    rust:palyra-auth) printf '%s\n' 'cargo test -p palyra-auth --locked' ;;
    rust:palyra-vault) printf '%s\n' 'cargo test -p palyra-vault --locked' ;;
    rust:palyra-browserd) printf '%s\n' 'cargo test -p palyra-browserd --locked' ;;
    rust:palyra-common) printf '%s\n' 'cargo test -p palyra-common --locked' ;;
    rust:palyra-transport-quic) printf '%s\n' 'cargo test -p palyra-transport-quic --locked' ;;
    rust:palyra-sandbox) printf '%s\n' 'cargo test -p palyra-sandbox --locked' ;;
    rust:palyra-skills) printf '%s\n' 'cargo test -p palyra-skills --locked' ;;
    web|contracts:web) printf '%s\n' 'npm --prefix apps/web run test:run' ;;
    desktop-ui) printf '%s\n' 'npm --prefix apps/desktop/ui run build' ;;
    desktop-tauri) printf '%s\n' 'cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --locked' ;;
    protocol|contracts) printf '%s\n' 'bash scripts/protocol/check-generated-stubs.sh' ;;
    ci|strict-gates) printf '%s\n' 'bash scripts/ci/plan-changed-scope.sh --self-test' ;;
    contracts:daemon|contracts:policy|contracts:cli|contracts:connectors|contracts:auth|contracts:browser|contracts:skills)
      printf '%s\n' 'bash scripts/test/check-runtime-contract-snapshots.sh'
      ;;
    security) printf '%s\n' 'bash scripts/check-high-risk-patterns.sh' ;;
    fuzz) printf '%s\n' 'cargo test --workspace --locked' ;;
    infra) printf '%s\n' 'bash scripts/check-high-risk-patterns.sh' ;;
    *) return 1 ;;
  esac
}

emit_plan() {
  local fallback_full="$1"
  local -n files_ref="$2"
  local -n scopes_ref="$3"
  local -n reasons_ref="$4"
  local -n commands_ref="$5"
  local profile="minimal"

  if [[ "$fallback_full" == true ]] || contains_item "strict-gates" "${scopes_ref[@]}"; then
    profile="strict"
  fi

  if [[ "$fallback_full" == true ]]; then
    commands_ref=(
      "cargo test --workspace --locked"
      "cargo clippy --workspace --all-targets -- -D warnings"
      "npm --prefix apps/web run test:run"
      "bash scripts/test/run-deterministic-core.sh"
    )
  fi

  printf '{'
  printf '"schema_version":1,'
  printf '"profile":%s,' "$(json_quote "$profile")"
  printf '"fallback_full":%s,' "$fallback_full"
  printf '"reason_codes":'; join_json_array "${reasons_ref[@]}"; printf ','
  printf '"changed_files":'; join_json_array "${files_ref[@]}"; printf ','
  printf '"scopes":'; join_json_array "${scopes_ref[@]}"; printf ','
  printf '"commands":'; join_json_array "${commands_ref[@]}"
  printf '}\n'
}

run_self_test() {
  local output
  output="$(PALYRA_CHANGED_SCOPE_FILES=$'crates/palyra-policy/src/lib.rs\napps/web/src/App.tsx' bash "$0")"
  grep -q '"rust:palyra-policy"' <<<"$output"
  grep -q '"web"' <<<"$output"
  grep -q '"fallback_full":false' <<<"$output"

  output="$(PALYRA_CHANGED_SCOPE_FILES='unclassified/path.bin' bash "$0")"
  grep -q '"fallback_full":true' <<<"$output"
  grep -q '"unclassified_path"' <<<"$output"
}

if [[ "${1:-}" == "--self-test" ]]; then
  run_self_test
  echo "changed scope planner self-test passed"
  exit 0
fi

cd "$ROOT_DIR"

if [[ ! -f "$CONFIG_PATH" ]]; then
  echo "changed scope map not found: $CONFIG_PATH" >&2
  exit 1
fi

declare -a files=()
declare -a scopes=()
declare -a reasons=()
declare -a commands=()
fallback_full=false

while IFS= read -r file || [[ -n "$file" ]]; do
  [[ -z "$file" ]] && continue
  append_unique_ref files "$file"
done < <(discover_changed_files)

if (( ${#files[@]} > MAX_CHANGED_FILES )); then
  fallback_full=true
  append_unique_ref reasons "too_many_changed_files"
fi

for file in "${files[@]}"; do
  matched=false
  while IFS=$'\t' read -r pattern raw_scopes strict_flag || [[ -n "${pattern:-}" ]]; do
    [[ -z "${pattern:-}" || "${pattern:0:1}" == "#" ]] && continue
    if [[ "$file" == $pattern ]]; then
      matched=true
      IFS=',' read -ra mapped_scopes <<<"$raw_scopes"
      for scope in "${mapped_scopes[@]}"; do
        append_unique_ref scopes "$scope"
      done
      if [[ "$strict_flag" == true ]]; then
        append_unique_ref scopes "strict-gates"
      fi
      break
    fi
  done < "$CONFIG_PATH"

  if [[ "$matched" == false ]]; then
    fallback_full=true
    append_unique_ref reasons "unclassified_path"
  fi
done

for scope in "${scopes[@]}"; do
  if command="$(command_for_scope "$scope")"; then
    append_unique_ref commands "$command"
  fi
done

emit_plan "$fallback_full" files scopes reasons commands
