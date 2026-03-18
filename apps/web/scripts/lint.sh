#!/usr/bin/env bash
set -euo pipefail

if ! command -v npm >/dev/null 2>&1; then
  echo "Web lint failed: npm is not installed." >&2
  exit 1
fi

bash scripts/test/ensure-js-workspace.sh

npm run web:lint
