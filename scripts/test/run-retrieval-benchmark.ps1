[CmdletBinding()]
param(
    [string]$Filter = "retrieval_eval_"
)

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
$rootDir = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
Set-Location $rootDir

if ($env:PALYRA_RETRIEVAL_BENCHMARK_FILTER) {
    $Filter = $env:PALYRA_RETRIEVAL_BENCHMARK_FILTER
}

cargo test -p palyra-daemon --lib $Filter --locked -- --test-threads=1
