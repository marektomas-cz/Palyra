#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/test/uninstall-clean-desktop.sh [options]

Options:
  --workspace-root <path>  Override the clean desktop harness root.
  --keep-artifacts         Preserve build/package artifacts and metadata.
  -h, --help               Show this help.

Linux defaults to ${XDG_DATA_HOME:-$HOME/.local/share}/Palyra-TestHarness.
USAGE
}

die() {
  echo "$*" >&2
  exit 1
}

require_value() {
  local flag="$1"
  local value="${2:-}"
  [[ -n "$value" ]] || die "$flag requires a value."
}

abs_path() {
  python3 -c 'import os, sys; print(os.path.abspath(sys.argv[1]))' "$1"
}

default_harness_root() {
  case "$(uname -s)" in
    Linux)
      printf '%s\n' "${XDG_DATA_HOME:-$HOME/.local/share}/Palyra-TestHarness"
      ;;
    Darwin)
      printf '%s\n' "$HOME/Library/Application Support/Palyra-TestHarness"
      ;;
    *)
      die "Unsupported operating system for clean desktop shell uninstall: $(uname -s)"
      ;;
  esac
}

remove_profile_block() {
  local profile_path="$1"
  python3 - "$profile_path" <<'PY'
import os
import re
import sys

profile_path = sys.argv[1]
try:
    with open(profile_path, encoding="utf-8") as fh:
        existing = fh.read()
except FileNotFoundError:
    raise SystemExit(0)
updated = re.sub(r"# >>> Palyra CLI >>>.*?# <<< Palyra CLI <<<\r?\n?", "", existing, flags=re.S).strip()
if updated == existing.strip():
    raise SystemExit(0)
if updated:
    with open(profile_path, "w", encoding="utf-8") as fh:
        fh.write(updated)
else:
    os.remove(profile_path)
print(profile_path)
PY
}

metadata_value() {
  local metadata_path="$1"
  local key="$2"
  [[ -f "$metadata_path" ]] || return 0
  python3 - "$metadata_path" "$key" <<'PY'
import json
import sys

path, key = sys.argv[1], sys.argv[2]
with open(path, encoding="utf-8") as fh:
    data = json.load(fh)
value = data.get(key)
if value is None:
    raise SystemExit(0)
if isinstance(value, bool):
    print("true" if value else "false")
else:
    print(value)
PY
}

stop_installed_process() {
  local binary_path="$1"
  [[ -e "$binary_path" ]] || return 0
  local expected pid exe target
  expected="$(readlink -f "$binary_path" 2>/dev/null || abs_path "$binary_path")"
  for exe in /proc/[0-9]*/exe; do
    target="$(readlink -f "$exe" 2>/dev/null || true)"
    [[ "$target" == "$expected" ]] || continue
    pid="${exe#/proc/}"
    pid="${pid%/exe}"
    kill "$pid" 2>/dev/null || true
  done
  sleep 1
  for exe in /proc/[0-9]*/exe; do
    target="$(readlink -f "$exe" 2>/dev/null || true)"
    [[ "$target" == "$expected" ]] || continue
    pid="${exe#/proc/}"
    pid="${pid%/exe}"
    kill -9 "$pid" 2>/dev/null || true
  done
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

command -v python3 >/dev/null 2>&1 || die "python3 is required for clean desktop uninstall testing."

workspace_root=""
keep_artifacts=false
while [[ $# -gt 0 ]]; do
  case "$1" in
    --workspace-root)
      require_value "$1" "${2:-}"
      workspace_root="$2"
      shift 2
      ;;
    --workspace-root=*)
      workspace_root="${1#*=}"
      shift
      ;;
    --keep-artifacts)
      keep_artifacts=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      die "Unknown argument: $1"
      ;;
  esac
done

workspace_root="$(abs_path "${workspace_root:-$(default_harness_root)}")"
artifacts_root="$workspace_root/artifacts"
install_root="$workspace_root/install"
state_root="$workspace_root/state"
metadata_path="$workspace_root/clean-install-metadata.json"
install_metadata_path="$install_root/install-metadata.json"
default_cli_command_root="$workspace_root/cli-bin"
cli_command_root="$default_cli_command_root"
metadata_cli_command_root="$(metadata_value "$metadata_path" "cli_command_root" || true)"
[[ -n "$metadata_cli_command_root" ]] && cli_command_root="$metadata_cli_command_root"

desktop_binary="$install_root/palyra-desktop-control-center"
daemon_binary="$install_root/palyrad"
browser_binary="$install_root/palyra-browserd"
for binary_path in "$desktop_binary" "$daemon_binary" "$browser_binary"; do
  stop_installed_process "$binary_path"
done

profile_files_raw="$(metadata_value "$install_metadata_path" "cli_profile_files" || true)"
IFS=':' read -r -a profile_files <<< "$profile_files_raw"
if [[ ${#profile_files[@]} -eq 0 || -z "${profile_files[0]:-}" ]]; then
  profile_files=("$HOME/.profile")
  [[ "$(uname -s)" == "Darwin" ]] && profile_files+=("$HOME/.zprofile")
  [[ -f "$HOME/.bash_profile" ]] && profile_files+=("$HOME/.bash_profile")
fi
cli_profile_updated=false
for profile_path in "${profile_files[@]}"; do
  [[ -n "$profile_path" ]] || continue
  if removed_profile="$(remove_profile_block "$profile_path")" && [[ -n "$removed_profile" ]]; then
    cli_profile_updated=true
  fi
done

cli_command_root_removed=false
if [[ -d "$cli_command_root" ]]; then
  rm -f "$cli_command_root/palyra"
  if ! find "$cli_command_root" -mindepth 1 -print -quit | grep -q .; then
    rmdir "$cli_command_root"
    cli_command_root_removed=true
  fi
fi
if [[ "$cli_command_root" != "$default_cli_command_root" && -d "$default_cli_command_root" ]]; then
  rm -f "$default_cli_command_root/palyra"
  if ! find "$default_cli_command_root" -mindepth 1 -print -quit | grep -q .; then
    rmdir "$default_cli_command_root"
  fi
fi

rm -rf "$install_root"
rm -rf "$state_root"
if [[ "$keep_artifacts" == false ]]; then
  rm -rf "$artifacts_root"
  rm -f "$metadata_path"
fi
if [[ -d "$workspace_root" ]] && ! find "$workspace_root" -mindepth 1 -print -quit | grep -q .; then
  rmdir "$workspace_root"
fi

echo "workspace_root=$workspace_root"
echo "install_root=$install_root"
echo "state_root=$state_root"
echo "cli_command_root=$cli_command_root"
echo "cli_command_root_removed=$cli_command_root_removed"
echo "cli_persistent_path_updated=$cli_profile_updated"
echo "cli_session_path_updated=false"
echo "artifacts_removed=$([[ "$keep_artifacts" == false ]] && echo true || echo false)"
