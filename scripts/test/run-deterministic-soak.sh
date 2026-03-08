#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cd "$ROOT_DIR"

cargo test -p palyra-connectors --lib --locked gateway_envelope_reconnect_resume_cycles_remain_stable_under_soak
cargo test -p palyra-connectors --lib --locked repeated_dead_letter_recovery_cycles_keep_queue_accounting_stable
