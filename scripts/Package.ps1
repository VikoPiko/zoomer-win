param([string]$Version = '0.2.0')
$ErrorActionPreference = 'Stop'
if ($Version -notmatch '^\d+\.\d+\.\d+([-.][A-Za-z0-9.-]+)?$') { throw 'Invalid package version' }
$root = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$binary = Join-Path $root 'target/x86_64-pc-windows-msvc/release/zoomer.exe'
if (-not (Test-Path -LiteralPath $binary)) { throw 'Run cargo build --release --locked --target x86_64-pc-windows-msvc first' }
if (-not (Test-Path -LiteralPath (Join-Path $root 'Cargo.lock'))) { throw 'A checked-in Cargo.lock is required for packaging' }
$manifestVersion = [regex]::Match((Get-Content -Raw -LiteralPath (Join-Path $root 'Cargo.toml')), '(?m)^version\s*=\s*"([^"]+)"').Groups[1].Value
if ($Version -ne $manifestVersion) { throw 'Package version must match Cargo.toml' }
$out = Join-Path $root 'dist'
New-Item -ItemType Directory -Force -Path $out | Out-Null
# Keep a directly runnable portable directory; never overwrite an existing bundle.
$stage = Join-Path $out "Zoomer-v$Version-windows-x64"
if (Test-Path -LiteralPath $stage) { throw "Portable directory already exists: $stage" }
New-Item -ItemType Directory -Path $stage | Out-Null
Copy-Item -LiteralPath $binary -Destination (Join-Path $stage 'Zoomer.exe')
foreach ($name in @('README.md', 'BUILD_RELEASE.md', 'Cargo.lock', 'config.example.toml', 'THIRD_PARTY_NOTICES.md')) {
    Copy-Item -LiteralPath (Join-Path $root $name) -Destination $stage
}
Copy-Item -LiteralPath (Join-Path $root 'docs') -Destination (Join-Path $stage 'docs') -Recurse
$zip = Join-Path $out "Zoomer-v$Version-windows-x64.zip"
if (Test-Path -LiteralPath $zip) { throw "Package already exists: $zip" }
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip -CompressionLevel Optimal
$hash = (Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant()
[System.IO.File]::WriteAllText("$zip.sha256", "$hash  $([System.IO.Path]::GetFileName($zip))`n")
Get-Item -LiteralPath $binary, $zip | Select-Object FullName, Length
Write-Output "SHA-256: $hash"
