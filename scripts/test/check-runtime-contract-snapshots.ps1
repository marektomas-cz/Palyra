[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
$rootDir = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
Set-Location $rootDir

cargo test -p palyra-policy explain_diagnostics_reports_safe_reason_code_and_hints --locked
cargo test -p palyra-daemon runtime_diagnostics::tests::contract_snapshot_suite_covers_phase11_abi_surfaces --locked
