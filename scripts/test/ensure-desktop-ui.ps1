[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
$rootDir = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
Set-Location $rootDir

if (-not (Test-Path (Join-Path $rootDir "apps\desktop\ui\node_modules"))) {
    npm --prefix apps/desktop/ui run bootstrap
} else {
    npm --prefix apps/desktop/ui run verify-install
}

npm --prefix apps/desktop/ui run build
