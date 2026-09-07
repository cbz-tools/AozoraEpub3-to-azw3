$ErrorActionPreference = 'Stop'
$crateName = 'aozora_epub3_to_azw3'
$version = (Select-String -LiteralPath 'Cargo.toml' -Pattern '^version = "([^"]+)"').Matches[0].Groups[1].Value
$forbidden = 'sovereign_stars_vol_1.epub', 'sovereign_stars_vol_1_cover.png', 'sovereign_stars_vol_1_page_001.png', 'sovereign_stars_vol_1_page_002.png', 'sovereign_stars_vol_1.txt', 'target/'
$required = @(
    'README.md',
    'README.ja.md',
    'CHANGELOG.md',
    'LICENSE',
    'THIRDPARTY_LICENSES.md'
)

$contents = cargo package --list --allow-dirty --locked
$normalizedContents = @($contents | ForEach-Object { $_.Replace('\', '/').TrimStart('./').ToLowerInvariant() })
foreach ($entry in $contents) {
    $normalized = $entry.Replace('\', '/').ToLowerInvariant()
    if ($forbidden | Where-Object { $normalized.Contains($_) }) {
        throw "forbidden package entry: $entry"
    }
}
foreach ($requiredEntry in $required) {
    $normalizedRequired = $requiredEntry.ToLowerInvariant()
    if ($normalizedContents -notcontains $normalizedRequired) {
        throw "required public package entry missing: $requiredEntry"
    }
}

cargo package --allow-dirty --locked
$crate = Join-Path 'target/package' "$crateName-$version.crate"
if (-not (Test-Path -LiteralPath $crate)) { throw "crate archive not found: $crate" }
$root = Join-Path 'target' 'package-verify'
if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
New-Item -ItemType Directory -Path $root | Out-Null
tar -xf $crate -C $root
$extracted = Join-Path $root "$crateName-$version"
if (-not (Test-Path -LiteralPath $extracted)) { throw "extracted crate not found: $extracted" }
$extractedRoot = [System.IO.Path]::GetFullPath($extracted).TrimEnd('\') + '\'
foreach ($readmeName in @('README.md', 'README.ja.md')) {
    $readmePath = Join-Path $extracted $readmeName
    $markdown = Get-Content -Raw -LiteralPath $readmePath
    $links = [regex]::Matches($markdown, '\]\(([^)\s]+)') |
        ForEach-Object { $_.Groups[1].Value } |
        Where-Object {
            $linkPath = $_.Split('#')[0].Replace('/', '\\')
            -not $linkPath.StartsWith('docs\\', [System.StringComparison]::OrdinalIgnoreCase)
        } |
        Select-Object -Unique
    foreach ($link in $links) {
        if ($link.StartsWith('#') -or $link -match '^[A-Za-z][A-Za-z0-9+.-]*:') {
            continue
        }
        $linkPath = $link.Split('#')[0]
        $resolved = [System.IO.Path]::GetFullPath(
            (Join-Path $extracted ($linkPath -replace '/', '\'))
        )
        if (-not $resolved.StartsWith($extractedRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
            throw "README link escapes extracted crate: $readmeName -> $link"
        }
        if (-not (Test-Path -LiteralPath $resolved -PathType Leaf)) {
            throw "README link does not resolve in extracted crate: $readmeName -> $link"
        }
    }
}
Push-Location $extracted
try { cargo test --all-features --locked } finally { Pop-Location }
Write-Output "Package verification passed: required public files are included, non-doc README links resolve, no Sovereign Stars fixture/assets or generated outputs are included, and extracted tests passed."
