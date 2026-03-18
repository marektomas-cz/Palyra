#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cd "$ROOT_DIR"

bash "$ROOT_DIR/scripts/test/ensure-js-workspace.sh"

npm run desktop-ui:build
