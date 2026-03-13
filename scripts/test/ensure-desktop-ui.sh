#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

cd "$ROOT_DIR"

if [[ ! -d "$ROOT_DIR/apps/desktop/ui/node_modules" ]]; then
  npm --prefix apps/desktop/ui run bootstrap
else
  npm --prefix apps/desktop/ui run verify-install
fi

npm --prefix apps/desktop/ui run build
