[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
$rootDir = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
Set-Location $rootDir

if (-not (Get-Command npm -ErrorAction SilentlyContinue)) {
    throw "npm is required for the root JS workspace."
}

if (-not (Test-Path (Join-Path $rootDir "node_modules"))) {
    Write-Output "Root JS workspace is missing; running npm ci to materialize local Vite+ binaries."
    npm ci
}

npm run verify:js-install | Out-Null
