$ErrorActionPreference = "Stop"

$existing = Get-Command vp -ErrorAction SilentlyContinue
if ($null -ne $existing) {
  Write-Host "vp already available at $($existing.Source)"
  vp --version
  exit 0
}

throw @"
vp is not available on PATH after setup.
Refusing to install vite-plus through the registry fallback because that bypasses lockfile integrity pinning in CI.
Ensure the pinned setup action provisions the vp CLI.
"@
