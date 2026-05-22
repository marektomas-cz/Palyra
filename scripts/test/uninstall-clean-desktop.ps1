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
        [string]$CommandRoot,
        [string[]]$AdditionalCommandRoots = @()
    )

    $sessionPathUpdated = $false
    $persistentPathUpdated = $false
    $commandRootRemoved = $false
    $commandRootPath = [IO.Path]::GetFullPath($CommandRoot)
    $candidateRoots = New-Object System.Collections.Generic.List[string]
    $candidateRoots.Add($commandRootPath) | Out-Null
    foreach ($additionalCommandRoot in $AdditionalCommandRoots) {
        if (-not [string]::IsNullOrWhiteSpace($additionalCommandRoot)) {
            $candidateRoots.Add([IO.Path]::GetFullPath($additionalCommandRoot)) | Out-Null
        }
    }
    foreach ($aliasRoot in (Get-WindowsPalyraCliAliasRoots)) {
        if (-not [string]::IsNullOrWhiteSpace($aliasRoot)) {
            $candidateRoots.Add($aliasRoot) | Out-Null
        }
    }

    foreach ($candidateRoot in ($candidateRoots | Select-Object -Unique)) {
        if (-not (Test-Path -LiteralPath $candidateRoot -PathType Container)) {
            continue
        }
        $shimNames =
            if ($IsWindows) {
                @("palyra.cmd", "palyra-pwsh.ps1", "palyra.ps1")
            } else {
                @("palyra")
            }

        foreach ($shimName in $shimNames) {
            $shimPath = Join-Path $candidateRoot $shimName
            if (Test-Path -LiteralPath $shimPath -PathType Leaf) {
                Remove-Item -LiteralPath $shimPath -Force
            }
        }

        $isWindowsAppsRoot = $false
        if ($IsWindows) {
            $localAppData = [Environment]::GetFolderPath("LocalApplicationData")
            if (-not [string]::IsNullOrWhiteSpace($localAppData)) {
                $isWindowsAppsRoot = Test-PathEntryEquals `
                    -Left $candidateRoot `
                    -Right (Join-Path $localAppData "Microsoft/WindowsApps")
            }
        }

        if ((-not $isWindowsAppsRoot) -and (Test-DirectoryEmpty -Path $candidateRoot)) {
            Remove-Item -LiteralPath $candidateRoot -Force
            if (Test-PathEntryEquals -Left $candidateRoot -Right $commandRootPath) {
                $commandRootRemoved = $true
            }
        }
    }

    if ($IsWindows) {
        foreach ($candidateRoot in ($candidateRoots | Select-Object -Unique)) {
            $localAppData = [Environment]::GetFolderPath("LocalApplicationData")
            $isWindowsAppsRoot = (-not [string]::IsNullOrWhiteSpace($localAppData)) -and
                (Test-PathEntryEquals -Left $candidateRoot -Right (Join-Path $localAppData "Microsoft/WindowsApps"))
            if ($isWindowsAppsRoot) {
                continue
            }
            $persistentPathUpdated = (Remove-WindowsUserPathEntry -Entry $candidateRoot) -or $persistentPathUpdated
        }
    }

    foreach ($candidateRoot in ($candidateRoots | Select-Object -Unique)) {
        if ($IsWindows) {
            $localAppData = [Environment]::GetFolderPath("LocalApplicationData")
            if (
                (-not [string]::IsNullOrWhiteSpace($localAppData)) -and
                (Test-PathEntryEquals -Left $candidateRoot -Right (Join-Path $localAppData "Microsoft/WindowsApps"))
            ) {
                continue
            }
        }
        $sessionPathUpdated = (Remove-CurrentSessionPathEntry -Entry $candidateRoot) -or $sessionPathUpdated
    }

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
$defaultCliCommandRoot = Join-Path $workspaceRoot "cli-bin"
$cliCommandRoot = $defaultCliCommandRoot
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

$cliCleanup = Remove-CleanDesktopCliExposureFallback `
    -CommandRoot $cliCommandRoot `
    -AdditionalCommandRoots @($defaultCliCommandRoot)
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
