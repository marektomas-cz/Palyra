#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

cargo test -p palyra-common replay_bundle --locked
cargo test -p palyra-cli support_bundle --locked
cargo test -p palyra-daemon replay_capture --locked
