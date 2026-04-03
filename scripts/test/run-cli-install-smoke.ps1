Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
. (Join-Path $repoRoot "scripts/release/common.ps1")

function Invoke-LoggedCommand {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Label,
        [Parameter(Mandatory = $true)]
        [string]$Command,
        [string[]]$Arguments = @(),
        [Parameter(Mandatory = $true)]
        [string]$LogPath
    )

    $invocationCommand = $Command
    $invocationArguments = $Arguments
    if ([string]::Equals([IO.Path]::GetExtension($Command), ".ps1", [StringComparison]::OrdinalIgnoreCase)) {
        $invocationCommand = "pwsh"
        $invocationArguments = @("-NoLogo", "-File", $Command) + $Arguments
    }

    $previousNativePreference = $PSNativeCommandUseErrorActionPreference
    $PSNativeCommandUseErrorActionPreference = $false
    try {
        $output = @(& $invocationCommand @invocationArguments 2>&1)
    }
    finally {
        $PSNativeCommandUseErrorActionPreference = $previousNativePreference
    }
    $exitCode = if ($null -eq $LASTEXITCODE) { 0 } else { $LASTEXITCODE }
    $text =
        if ($output.Count -eq 0) {
            ""
        } else {
            (($output | ForEach-Object { $_.ToString() }) -join [Environment]::NewLine)
        }
    Set-Content -LiteralPath $LogPath -Value $text
    if ($exitCode -ne 0) {
        throw "$Label failed with exit code $exitCode. See $LogPath."
    }

    return ,$output
}

function Assert-RemovedPath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path,
        [Parameter(Mandatory = $true)]
        [string]$Label
    )

    if (Test-Path -LiteralPath $Path) {
        throw "$Label should be removed but still exists: $Path"
    }
}

$outputRoot = Join-Path $repoRoot "target/release-artifacts/cli-install-smoke"
if (Test-Path -LiteralPath $outputRoot) {
    Remove-Item -LiteralPath $outputRoot -Recurse -Force
}
New-Item -ItemType Directory -Path $outputRoot -Force | Out-Null
$logsRoot = Join-Path $outputRoot "logs"
New-Item -ItemType Directory -Path $logsRoot -Force | Out-Null

$version = (& (Join-Path $repoRoot "scripts/release/assert-version-coherence.ps1")).Trim()
$platform = Get-PlatformSlug
$headlessPackageOutput = Join-Path $outputRoot "headless"
$installRoot = Join-Path $outputRoot "installed-headless"
$configPath = Join-Path $outputRoot "installed-headless-config/palyra.toml"
$stateRoot = Join-Path $outputRoot "installed-headless-state"
$cliCommandRoot = Join-Path $outputRoot "cli-bin"
$archivePath = Join-Path $headlessPackageOutput "palyra-headless-$version-$platform.zip"
$installMetadataPath = Join-Path $installRoot "install-metadata.json"

Push-Location $repoRoot
try {
    Invoke-LoggedCommand `
        -Label "ensure-web-ui" `
        -Command (Join-Path $repoRoot "scripts/test/ensure-web-ui.ps1") `
        -LogPath (Join-Path $logsRoot "ensure-web-ui.log") | Out-Null

    Invoke-LoggedCommand `
        -Label "cargo-build-release" `
        -Command "cargo" `
        -Arguments @("build", "-p", "palyra-daemon", "-p", "palyra-browserd", "-p", "palyra-cli", "--release", "--locked") `
        -LogPath (Join-Path $logsRoot "cargo-build-release.log") | Out-Null

    $daemonBinary = Join-Path $repoRoot ("target/release/" + (Resolve-ExecutableName -BaseName "palyrad"))
    $browserBinary = Join-Path $repoRoot ("target/release/" + (Resolve-ExecutableName -BaseName "palyra-browserd"))
    $cliBinary = Join-Path $repoRoot ("target/release/" + (Resolve-ExecutableName -BaseName "palyra"))
    $webDist = Join-Path $repoRoot "apps/web/dist"

    Invoke-LoggedCommand `
        -Label "package-headless" `
        -Command (Join-Path $repoRoot "scripts/release/package-portable.ps1") `
        -Arguments @(
            "-ArtifactKind", "headless",
            "-Version", $version,
            "-OutputRoot", $headlessPackageOutput,
            "-DaemonBinaryPath", $daemonBinary,
            "-BrowserBinaryPath", $browserBinary,
            "-CliBinaryPath", $cliBinary,
            "-WebDistPath", $webDist
        ) `
        -LogPath (Join-Path $logsRoot "package-headless.log") | Out-Null

    Invoke-LoggedCommand `
        -Label "validate-headless-archive" `
        -Command (Join-Path $repoRoot "scripts/release/validate-portable-archive.ps1") `
        -Arguments @("-Path", $archivePath, "-ExpectedArtifactKind", "headless") `
        -LogPath (Join-Path $logsRoot "validate-headless-archive.log") | Out-Null

    $installOutput = Invoke-LoggedCommand `
        -Label "install-headless-package" `
        -Command (Join-Path $repoRoot "scripts/release/install-headless-package.ps1") `
        -Arguments @(
            "-ArchivePath", $archivePath,
            "-InstallRoot", $installRoot,
            "-ConfigPath", $configPath,
            "-StateRoot", $stateRoot,
            "-CliCommandRoot", $cliCommandRoot,
            "-NoPersistCliPath",
            "-Force",
            "-SkipSystemdUnit:$IsWindows"
        ) `
        -LogPath (Join-Path $logsRoot "install-headless-package.log")
    $installMetadata = Convert-KeyValueOutputToHashtable -Lines $installOutput
    $installMetadata | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $logsRoot "install-headless-package.json")

    $installManifest = Read-JsonFile -Path $installMetadataPath
    $binaryUnderTest = [string]$installManifest.cli_exposure.target_binary_path
    if ([string]::IsNullOrWhiteSpace($binaryUnderTest)) {
        throw "install metadata did not expose cli_exposure.target_binary_path"
    }

    $summary = [ordered]@{
        version = $version
        platform = $platform
        archive_path = $archivePath
        install_root = $installRoot
        config_path = $configPath
        state_root = $stateRoot
        cli_command_root = $installMetadata["cli_command_root"]
        cli_command_path = $installMetadata["cli_command_path"]
        binary_under_test = $binaryUnderTest
    }
    $summary | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $outputRoot "summary.json")

    $previousBinary = $env:PALYRA_BIN_UNDER_TEST
    $previousArchive = $env:PALYRA_INSTALL_ARCHIVE_PATH
    $previousInstallRoot = $env:PALYRA_INSTALL_ROOT
    $previousConfig = $env:PALYRA_CONFIG_UNDER_TEST
    $previousStateRoot = $env:PALYRA_STATE_ROOT_UNDER_TEST
    try {
        $env:PALYRA_BIN_UNDER_TEST = $binaryUnderTest
        $env:PALYRA_INSTALL_ARCHIVE_PATH = $archivePath
        $env:PALYRA_INSTALL_ROOT = $installRoot
        $env:PALYRA_CONFIG_UNDER_TEST = $configPath
        $env:PALYRA_STATE_ROOT_UNDER_TEST = $stateRoot

        Invoke-LoggedCommand `
            -Label "cargo-test-installed-smoke" `
            -Command "cargo" `
            -Arguments @("test", "-p", "palyra-cli", "--test", "installed_smoke", "--locked", "--", "--test-threads=1") `
            -LogPath (Join-Path $logsRoot "cargo-test-installed-smoke.log") | Out-Null
    }
    finally {
        if ($null -eq $previousBinary) { Remove-Item Env:PALYRA_BIN_UNDER_TEST -ErrorAction SilentlyContinue } else { $env:PALYRA_BIN_UNDER_TEST = $previousBinary }
        if ($null -eq $previousArchive) { Remove-Item Env:PALYRA_INSTALL_ARCHIVE_PATH -ErrorAction SilentlyContinue } else { $env:PALYRA_INSTALL_ARCHIVE_PATH = $previousArchive }
        if ($null -eq $previousInstallRoot) { Remove-Item Env:PALYRA_INSTALL_ROOT -ErrorAction SilentlyContinue } else { $env:PALYRA_INSTALL_ROOT = $previousInstallRoot }
        if ($null -eq $previousConfig) { Remove-Item Env:PALYRA_CONFIG_UNDER_TEST -ErrorAction SilentlyContinue } else { $env:PALYRA_CONFIG_UNDER_TEST = $previousConfig }
        if ($null -eq $previousStateRoot) { Remove-Item Env:PALYRA_STATE_ROOT_UNDER_TEST -ErrorAction SilentlyContinue } else { $env:PALYRA_STATE_ROOT_UNDER_TEST = $previousStateRoot }
    }
}
finally {
    try {
        if (Test-Path -LiteralPath $installRoot) {
            $uninstallOutput = Invoke-LoggedCommand `
                -Label "uninstall-headless-package" `
                -Command (Join-Path $repoRoot "scripts/release/uninstall-package.ps1") `
                -Arguments @("-InstallRoot", $installRoot, "-RemoveStateRoot") `
                -LogPath (Join-Path $logsRoot "uninstall-headless-package.log")
            $uninstallMetadata = Convert-KeyValueOutputToHashtable -Lines $uninstallOutput
            $uninstallMetadata | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $logsRoot "uninstall-headless-package.json")

            Assert-RemovedPath -Path $installRoot -Label "install root"
            Assert-RemovedPath -Path $stateRoot -Label "state root"
            if (Test-Path -LiteralPath $cliCommandRoot -PathType Container) {
                if (-not (Test-DirectoryEmpty -Path $cliCommandRoot)) {
                    throw "CLI command root should be empty after uninstall cleanup: $cliCommandRoot"
                }
            }
        }
    }
    finally {
        Pop-Location
    }
}

Write-Output "cli_install_smoke=passed"
Write-Output "version=$version"
Write-Output "platform=$platform"
Write-Output "archive_path=$archivePath"
Write-Output "summary_path=$(Join-Path $outputRoot "summary.json")"
