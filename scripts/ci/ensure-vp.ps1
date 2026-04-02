$ErrorActionPreference = "Stop"

$existing = Get-Command vp -ErrorAction SilentlyContinue
if ($null -ne $existing) {
  Write-Host "vp already available at $($existing.Source)"
  vp --version
  exit 0
}

$vpVersion = node -p "require('./package.json').devDependencies['vite-plus']"
if ([string]::IsNullOrWhiteSpace($vpVersion)) {
  throw "Unable to resolve vite-plus version from package.json."
}

Write-Host "vp not found on PATH; installing vite-plus@$vpVersion globally as a workflow fallback."
npm install --global "vite-plus@$vpVersion"

$globalPrefix = (npm prefix -g).Trim()
$globalBin = if ($IsWindows) { $globalPrefix } else { Join-Path $globalPrefix "bin" }

if (-not (Test-Path $globalBin)) {
  throw "Global npm bin path '$globalBin' does not exist after vite-plus installation."
}

$env:PATH =
  if ($IsWindows) {
    "${globalBin};$env:PATH"
  } else {
    "${globalBin}:$env:PATH"
  }

if ($env:GITHUB_PATH) {
  $globalBin | Out-File -FilePath $env:GITHUB_PATH -Encoding utf8 -Append
}

$resolved = Get-Command vp -ErrorAction SilentlyContinue
if ($null -eq $resolved) {
  throw "vp is still unavailable after fallback installation."
}

Write-Host "vp fallback installed at $($resolved.Source)"
vp --version
