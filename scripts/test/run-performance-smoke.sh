#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

resolve_cargo() {
  if command -v cargo >/dev/null 2>&1; then
    command -v cargo
    return 0
  fi
  if command -v cargo.exe >/dev/null 2>&1; then
    command -v cargo.exe
    return 0
  fi

  local candidates=(
    "${HOME:-}/.cargo/bin/cargo"
    "${HOME:-}/.cargo/bin/cargo.exe"
    "${USERPROFILE:-}/.cargo/bin/cargo.exe"
  )
  local candidate
  for candidate in "${candidates[@]}"; do
    if [[ -n "$candidate" && -x "$candidate" ]]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done

  echo "cargo is required for performance smoke checks." >&2
  exit 1
}

cd "$ROOT_DIR"

CARGO_BIN="$(resolve_cargo)"

if [[ ! -d "$ROOT_DIR/apps/web/node_modules" ]]; then
  npm --prefix apps/web run bootstrap
else
  npm --prefix apps/web run verify-install
fi

"$CARGO_BIN" test -p palyra-daemon --locked retention_housekeeping
"$CARGO_BIN" test -p palyra-auth --locked refresh_due_profiles_marks_transport_failure_without_retry_spam
"$CARGO_BIN" test --manifest-path apps/desktop/src-tauri/Cargo.toml --locked desktop_refresh_payload_reuses_single_snapshot_build_for_home_and_onboarding_views
"$CARGO_BIN" test --manifest-path apps/desktop/src-tauri/Cargo.toml --locked support_bundle_export_plan_capture_does_not_hold_supervisor_lock

npm --prefix apps/web run perf:smoke
