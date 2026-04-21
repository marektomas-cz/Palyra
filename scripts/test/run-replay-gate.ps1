[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
$rootDir = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
Set-Location $rootDir

cargo test -p palyra-common replay_bundle --locked
cargo test -p palyra-cli support_bundle --locked
cargo test -p palyra-daemon replay_capture --locked
