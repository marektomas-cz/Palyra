#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cd "$ROOT_DIR"

PALYRA_REFRESH_DETERMINISTIC_FIXTURES=1 cargo test -p palyra-connectors --test simulator_harness --locked
