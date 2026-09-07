$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$manifestPath = Join-Path $repoRoot 'Cargo.toml'
$generatorPath = Join-Path $PSScriptRoot 'gen-thirdparty-licenses.py'
$licensePath = Join-Path $repoRoot 'THIRDPARTY_LICENSES.md'
$verifyPackagePath = Join-Path $PSScriptRoot 'verify-package.ps1'
$pinnedCargoLicenseVersion = '0.6.1'

function Invoke-CheckedNativeCommand {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][string]$Executable,
        [Parameter(Mandatory = $false)][string[]]$Arguments = @()
    )

    Write-Host "==> $Name"
    & $Executable @Arguments
    $exitCode = $LASTEXITCODE
    if ($exitCode -ne 0) {
        throw "$Name failed with exit code $exitCode."
    }
}

function Get-CargoLicenseInstalledVersion {
    $listOutput = & cargo install --list 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "Could not inspect installed Cargo tools with 'cargo install --list'."
    }
    foreach ($line in $listOutput) {
        $match = [regex]::Match([string]$line, '^\s*cargo-license v(?<version>[^:]+):\s*$')
        if ($match.Success) {
            return $match.Groups['version'].Value
        }
    }
    return $null
}

function Get-CargoLicensePath {
    $command = Get-Command cargo-license -CommandType Application -ErrorAction SilentlyContinue
    if ($null -eq $command) {
        return $null
    }
    return $command.Source
}

function Ensure-CargoLicense {
    $existingPath = Get-CargoLicensePath
    $existingVersion = Get-CargoLicenseInstalledVersion
    if ($null -ne $existingPath -and $existingVersion -eq $pinnedCargoLicenseVersion) {
        Write-Host "Using cargo-license $pinnedCargoLicenseVersion at $existingPath"
        return $existingPath
    }

    if ($null -eq $existingPath) {
        Write-Host "cargo-license is not available; installing pinned version $pinnedCargoLicenseVersion."
    } else {
        $reportedVersion = if ($null -eq $existingVersion) { 'unknown' } else { $existingVersion }
        Write-Warning "cargo-license at $existingPath reports version $reportedVersion; installing pinned version $pinnedCargoLicenseVersion."
    }

    $installArguments = @('install', 'cargo-license', '--version', $pinnedCargoLicenseVersion, '--locked')
    if ($null -ne $existingPath -or $null -ne $existingVersion) {
        $installArguments += '--force'
    }
    Invoke-CheckedNativeCommand -Name "Install cargo-license $pinnedCargoLicenseVersion" -Executable 'cargo' -Arguments $installArguments

    $installedPath = Get-CargoLicensePath
    $installedVersion = Get-CargoLicenseInstalledVersion
    if ($null -eq $installedPath -or $installedVersion -ne $pinnedCargoLicenseVersion) {
        throw "cargo-license remediation did not produce required version $pinnedCargoLicenseVersion."
    }
    Write-Host "Using cargo-license $installedVersion at $installedPath"
    return $installedPath
}

function Get-PythonInvocation {
    $launcher = Get-Command py -CommandType Application -ErrorAction SilentlyContinue
    if ($null -ne $launcher) {
        return @{ Executable = $launcher.Source; Prefix = @('-3') }
    }
    $python = Get-Command python -CommandType Application -ErrorAction SilentlyContinue
    if ($null -ne $python) {
        return @{ Executable = $python.Source; Prefix = @() }
    }
    throw 'Python 3.11 or newer is required, but neither py nor python was found.'
}

function Update-ThirdPartyLicenses {
    param([Parameter(Mandatory = $true)][string]$CargoLicensePath)

    $temporaryDirectory = Join-Path ([IO.Path]::GetTempPath()) ('aozoraepub3-to-azw3-license-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $temporaryDirectory -Force | Out-Null
    $jsonPath = Join-Path $temporaryDirectory 'cargo-license.json'
    $stderrPath = Join-Path $temporaryDirectory 'cargo-license.stderr'
    try {
        Write-Host '==> Generate cargo-license JSON metadata'
        $jsonLines = & $CargoLicensePath '--json' '--direct-deps-only' '--all-features' '--manifest-path' $manifestPath '--color' 'never' 2> $stderrPath
        $cargoLicenseExitCode = $LASTEXITCODE
        $stderr = if (Test-Path -LiteralPath $stderrPath) { [IO.File]::ReadAllText($stderrPath) } else { '' }
        if ($cargoLicenseExitCode -ne 0) {
            throw "cargo-license failed with exit code $cargoLicenseExitCode.`n$stderr"
        }
        $jsonLines = @($jsonLines)
        if ($jsonLines.Count -eq 0) {
            throw 'cargo-license returned empty JSON output.'
        }
        [IO.File]::WriteAllText($jsonPath, ($jsonLines -join [Environment]::NewLine), [Text.UTF8Encoding]::new($false))

        $python = Get-PythonInvocation
        $pythonArguments = @($python.Prefix) + @($generatorPath, '--input', $jsonPath, '--manifest-path', $manifestPath, '--output', $licensePath)
        Invoke-CheckedNativeCommand -Name 'Regenerate THIRDPARTY_LICENSES.md' -Executable $python.Executable -Arguments $pythonArguments
    } finally {
        if (Test-Path -LiteralPath $temporaryDirectory) {
            Remove-Item -LiteralPath $temporaryDirectory -Recurse -Force
        }
    }
}

$exitCode = 0
$locationPushed = $false
try {
    foreach ($requiredPath in @($manifestPath, $generatorPath, $verifyPackagePath)) {
        if (-not (Test-Path -LiteralPath $requiredPath -PathType Leaf)) {
            throw "Required repository file not found: $requiredPath"
        }
    }
    Push-Location -LiteralPath $repoRoot
    $locationPushed = $true

    $cargoLicensePath = Ensure-CargoLicense
    Update-ThirdPartyLicenses -CargoLicensePath $cargoLicensePath
    Invoke-CheckedNativeCommand -Name 'cargo fmt check' -Executable 'cargo' -Arguments @('fmt', '--all', '--', '--check')
    Invoke-CheckedNativeCommand -Name 'cargo check' -Executable 'cargo' -Arguments @('check', '--all-targets', '--all-features', '--locked')
    Invoke-CheckedNativeCommand -Name 'cargo clippy' -Executable 'cargo' -Arguments @('clippy', '--all-targets', '--all-features', '--locked', '--', '-D', 'warnings')
    Invoke-CheckedNativeCommand -Name 'cargo test' -Executable 'cargo' -Arguments @('test', '--all-features', '--locked')

    $oldRustdocFlags = $env:RUSTDOCFLAGS
    try {
        $env:RUSTDOCFLAGS = '-D warnings'
        Invoke-CheckedNativeCommand -Name 'cargo doc' -Executable 'cargo' -Arguments @('doc', '--no-deps', '--locked')
    } finally {
        $env:RUSTDOCFLAGS = $oldRustdocFlags
    }

    Invoke-CheckedNativeCommand -Name 'cargo build (release)' -Executable 'cargo' -Arguments @('build', '--release', '--all-features', '--locked')
    Write-Host '==> Verify package contents'
    & $verifyPackagePath
    $verifyExitCode = $LASTEXITCODE
    if ($verifyExitCode -ne 0) {
        throw "Package verification failed with exit code $verifyExitCode."
    }
    if (-not $?) {
        throw 'Package verification failed.'
    }
} catch {
    Write-Error $_
    $exitCode = 1
} finally {
    if ($locationPushed) {
        Pop-Location
    }
}
exit $exitCode
