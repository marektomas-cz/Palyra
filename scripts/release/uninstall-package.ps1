param(
    [Parameter(Mandatory = $true)]
    [string]$InstallRoot,
    [switch]$RemoveStateRoot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

. "$PSScriptRoot/common.ps1"

$resolvedInstallRoot = [IO.Path]::GetFullPath($InstallRoot)
$metadataPath = Join-Path $resolvedInstallRoot "install-metadata.json"

function Get-MetadataPropertyValue {
    param(
        [Parameter(Mandatory = $true)]
        [object]$Metadata,
        [Parameter(Mandatory = $true)]
        [string]$Name
    )

    $property = $Metadata.PSObject.Properties[$Name]
    if ($null -eq $property) {
        return $null
    }

    return $property.Value
}

$stateRoot = $null
$cliCleanup = $null
if (Test-Path -LiteralPath $metadataPath -PathType Leaf) {
    $metadata = Read-JsonFile -Path $metadataPath
    $metadataStateRoot = Get-MetadataPropertyValue -Metadata $metadata -Name "state_root"
    if ($null -ne $metadataStateRoot -and -not [string]::IsNullOrWhiteSpace([string]$metadataStateRoot)) {
        $stateRoot = [string]$metadataStateRoot
    }

    $metadataCliExposure = Get-MetadataPropertyValue -Metadata $metadata -Name "cli_exposure"
    if ($null -ne $metadataCliExposure) {
        $cliCleanup = Remove-PalyraCliExposure -CliExposure $metadataCliExposure
    }
}

if (Test-Path -LiteralPath $resolvedInstallRoot) {
    Remove-Item -LiteralPath $resolvedInstallRoot -Recurse -Force
}

$stateRootRemoved = $false
if ($RemoveStateRoot -and -not [string]::IsNullOrWhiteSpace($stateRoot) -and (Test-Path -LiteralPath $stateRoot)) {
    Remove-Item -LiteralPath $stateRoot -Recurse -Force
    $stateRootRemoved = $true
}

Write-Output "install_root=$resolvedInstallRoot"
Write-Output "install_root_removed=$true"
Write-Output "state_root=$stateRoot"
Write-Output "state_root_removed=$stateRootRemoved"
if ($null -ne $cliCleanup) {
    Write-Output "cli_command_root_removed=$($cliCleanup.command_root_removed)"
}
