$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$manifestPath = Join-Path $repoRoot 'Cargo.toml'
$verifyPackagePath = Join-Path $PSScriptRoot 'verify-package.ps1'

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

$exitCode = 0
$locationPushed = $false
try {
    foreach ($requiredPath in @($manifestPath, $verifyPackagePath)) {
        if (-not (Test-Path -LiteralPath $requiredPath -PathType Leaf)) {
            throw "Required repository file not found: $requiredPath"
        }
    }
    Push-Location -LiteralPath $repoRoot
    $locationPushed = $true

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
