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
  echo "cargo is required for runtime contract snapshot checks." >&2
  exit 1
}

cd "$ROOT_DIR"
CARGO_BIN="$(resolve_cargo)"

"$CARGO_BIN" test -p palyra-policy explain_diagnostics_reports_safe_reason_code_and_hints --locked
"$CARGO_BIN" test -p palyra-daemon runtime_diagnostics::tests::contract_snapshot_suite_covers_phase11_abi_surfaces --locked
