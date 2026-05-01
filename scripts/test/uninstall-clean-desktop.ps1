param(
    [string]$WorkspaceRoot,
    [switch]$KeepArtifacts
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

. (Join-Path $PSScriptRoot "../release/common.ps1")

function Get-DefaultHarnessRoot {
    $localAppData = [Environment]::GetFolderPath("LocalApplicationData")
    if ([string]::IsNullOrWhiteSpace($localAppData)) {
        throw "Unable to resolve LocalApplicationData for the clean desktop test harness."
    }

    return Join-Path $localAppData "Palyra-TestHarness"
}

function Stop-InstalledProcess {
    param(
        [Parameter(Mandatory = $true)]
        [string]$ExecutablePath
    )

    $expectedPath = [IO.Path]::GetFullPath($ExecutablePath)
    $comparison = [StringComparison]::OrdinalIgnoreCase
    Get-Process -ErrorAction SilentlyContinue |
        Where-Object {
            try {
                -not [string]::IsNullOrWhiteSpace($_.Path) -and
                    [string]::Equals([IO.Path]::GetFullPath($_.Path), $expectedPath, $comparison)
            } catch {
                $false
            }
        } |
        ForEach-Object {
            Stop-Process -Id $_.Id -Force -ErrorAction Stop
        }
}

function Remove-CleanDesktopCliExposureFallback {
    param(
        [Parameter(Mandatory = $true)]
        [string]$CommandRoot
    )

    $sessionPathUpdated = $false
    $persistentPathUpdated = $false
    $commandRootRemoved = $false
    $commandRootPath = [IO.Path]::GetFullPath($CommandRoot)

    if (Test-Path -LiteralPath $commandRootPath -PathType Container) {
        $shimNames =
            if ($IsWindows) {
                @("palyra.cmd", "palyra.ps1")
            } else {
                @("palyra")
            }

        foreach ($shimName in $shimNames) {
            $shimPath = Join-Path $commandRootPath $shimName
            if (Test-Path -LiteralPath $shimPath -PathType Leaf) {
                Remove-Item -LiteralPath $shimPath -Force
            }
        }

        if (Test-DirectoryEmpty -Path $commandRootPath) {
            Remove-Item -LiteralPath $commandRootPath -Force
            $commandRootRemoved = $true
        }
    }

    if ($IsWindows) {
        $persistentPathUpdated = Remove-WindowsUserPathEntry -Entry $commandRootPath
    }

    $sessionPathUpdated = Remove-CurrentSessionPathEntry -Entry $commandRootPath

    return [ordered]@{
        command_root_removed = $commandRootRemoved
        persistent_path_updated = $persistentPathUpdated
        session_path_updated = $sessionPathUpdated
    }
}

$workspaceRoot =
    if ([string]::IsNullOrWhiteSpace($WorkspaceRoot)) {
        Get-DefaultHarnessRoot
    } else {
        [IO.Path]::GetFullPath($WorkspaceRoot)
    }

$artifactsRoot = Join-Path $workspaceRoot "artifacts"
$installRoot = Join-Path $workspaceRoot "install"
$stateRoot = Join-Path $workspaceRoot "state"
$metadataPath = Join-Path $workspaceRoot "clean-install-metadata.json"
$cliCommandRoot = Join-Path $workspaceRoot "cli-bin"
if (Test-Path -LiteralPath $metadataPath -PathType Leaf) {
    $cleanMetadata = Read-JsonFile -Path $metadataPath
    $metadataCliCommandRoot = $cleanMetadata.PSObject.Properties["cli_command_root"]
    if (
        $null -ne $metadataCliCommandRoot -and
        -not [string]::IsNullOrWhiteSpace([string]$metadataCliCommandRoot.Value)
    ) {
        $cliCommandRoot = [string]$metadataCliCommandRoot.Value
    }
}

$desktopBinary =
    Join-Path $installRoot (Resolve-ExecutableName -BaseName "palyra-desktop-control-center")
$daemonBinary = Join-Path $installRoot (Resolve-ExecutableName -BaseName "palyrad")
$browserBinary = Join-Path $installRoot (Resolve-ExecutableName -BaseName "palyra-browserd")

foreach ($binaryPath in @($desktopBinary, $daemonBinary, $browserBinary)) {
    Stop-InstalledProcess -ExecutablePath $binaryPath
}

$uninstallMetadata = @{}
if (Test-Path -LiteralPath $installRoot) {
    $uninstallOutput = & (Join-Path $PSScriptRoot "../release/uninstall-package.ps1") `
        -InstallRoot $installRoot `
        -RemoveStateRoot
    $uninstallMetadata = Convert-KeyValueOutputToHashtable -Lines $uninstallOutput
}

$cliCleanup = Remove-CleanDesktopCliExposureFallback -CommandRoot $cliCommandRoot
$cliCommandRootRemoved = $uninstallMetadata["cli_command_root_removed"]
if ([string]::IsNullOrWhiteSpace($cliCommandRootRemoved)) {
    $cliCommandRootRemoved = [string]$cliCleanup.command_root_removed
}

if (Test-Path -LiteralPath $stateRoot) {
    Remove-Item -LiteralPath $stateRoot -Recurse -Force
}

if ((Test-Path -LiteralPath $artifactsRoot) -and -not $KeepArtifacts) {
    Remove-Item -LiteralPath $artifactsRoot -Recurse -Force
}

if ((Test-Path -LiteralPath $metadataPath) -and -not $KeepArtifacts) {
    Remove-Item -LiteralPath $metadataPath -Force
}

if ((Test-Path -LiteralPath $workspaceRoot) -and -not (Get-ChildItem -LiteralPath $workspaceRoot -Force | Select-Object -First 1)) {
    Remove-Item -LiteralPath $workspaceRoot -Force
}

Write-Output "workspace_root=$workspaceRoot"
Write-Output "install_root=$installRoot"
Write-Output "state_root=$stateRoot"
Write-Output "cli_command_root=$cliCommandRoot"
Write-Output "cli_command_root_removed=$cliCommandRootRemoved"
Write-Output "cli_persistent_path_updated=$($cliCleanup.persistent_path_updated)"
Write-Output "cli_session_path_updated=$($cliCleanup.session_path_updated)"
Write-Output "artifacts_removed=$($KeepArtifacts -eq $false)"
