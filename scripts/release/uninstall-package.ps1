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
$desktopAutostartAppName = "Palyra Control Center"

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

function Remove-RegistryValueIfPresent {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,
        [Parameter(Mandatory = $true)]
        [string]$Name
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        return $false
    }

    $properties = Get-ItemProperty -LiteralPath $Path -ErrorAction SilentlyContinue
    if ($null -eq $properties -or $null -eq $properties.PSObject.Properties[$Name]) {
        return $false
    }

    Remove-ItemProperty -LiteralPath $Path -Name $Name -ErrorAction Stop
    return $true
}

function Remove-DesktopAutostartEntry {
    param(
        [Parameter(Mandatory = $true)]
        [string]$AppName
    )

    $removed = $false
    if ($IsWindows) {
        $runKey = "HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Run"
        $startupApprovedKey = "HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run"
        $removed = (Remove-RegistryValueIfPresent -Path $runKey -Name $AppName) -or $removed
        $removed = (Remove-RegistryValueIfPresent -Path $startupApprovedKey -Name $AppName) -or $removed
        return $removed
    }

    if ($IsMacOS) {
        $launchAgentPath = Join-Path $HOME "Library/LaunchAgents/$AppName.plist"
        if (Test-Path -LiteralPath $launchAgentPath -PathType Leaf) {
            Remove-Item -LiteralPath $launchAgentPath -Force
            $removed = $true
        }
        return $removed
    }

    $linuxAutostartPath = Join-Path $HOME ".config/autostart/$AppName.desktop"
    if (Test-Path -LiteralPath $linuxAutostartPath -PathType Leaf) {
        Remove-Item -LiteralPath $linuxAutostartPath -Force
        $removed = $true
    }
    return $removed
}

$stateRoot = $null
$cliCleanup = $null
$desktopAutostartRemoved = $false
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

    $artifactKind = Get-MetadataPropertyValue -Metadata $metadata -Name "artifact_kind"
    if (
        $null -ne $artifactKind -and
        [string]::Equals([string]$artifactKind, "desktop", [StringComparison]::OrdinalIgnoreCase)
    ) {
        $desktopAutostartRemoved = Remove-DesktopAutostartEntry -AppName $desktopAutostartAppName
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
Write-Output "desktop_autostart_removed=$desktopAutostartRemoved"
