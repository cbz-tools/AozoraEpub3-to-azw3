$ErrorActionPreference = 'Stop'

$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$manifestPath = Join-Path $repoRoot 'Cargo.toml'

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
    foreach ($requiredPath in @($manifestPath)) {
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
    Invoke-CheckedNativeCommand -Name 'cargo build (debug)' -Executable 'cargo' -Arguments @('build', '--all-features', '--locked')
} catch {
    Write-Error $_
    $exitCode = 1
} finally {
    if ($locationPushed) {
        Pop-Location
    }
}
exit $exitCode
