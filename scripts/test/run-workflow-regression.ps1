Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)

Push-Location $repoRoot
try {
    cargo build -p palyra-daemon -p palyra-cli --locked
    cargo build -p palyra-browserd --bin palyra-browserd --locked

    cargo test -p palyra-daemon --lib --locked compat::tests
    cargo test -p palyra-cli --test wizard_cli --locked -- --test-threads=1
    cargo test -p palyra-cli --test cli_v1_acp_shim --locked -- --test-threads=1
    cargo test -p palyra-cli --test workflow_regression_matrix --locked -- --test-threads=1
}
finally {
    Pop-Location
}
