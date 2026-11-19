# Builds a license-free portable package and a ZIP that contains setup.exe.
# No third-party installer tooling or license key is required.
param(
    [switch]$DebugBuild
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$profile = if ($DebugBuild) { 'debug' } else { 'release' }
$binDir = Join-Path $root "target\$profile"

if (-not (Test-Path (Join-Path $root 'Cargo.toml'))) {
    throw "Cargo workspace not found at $root"
}

Push-Location $root
try {
    if ($DebugBuild) {
        cargo build --workspace
    } else {
        cargo build --release --workspace
    }
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
}
finally {
    Pop-Location
}

$package = Join-Path $root 'installer-package'
New-Item -ItemType Directory -Force -Path $package | Out-Null
foreach ($name in @('atk-hub-rs.exe', 'atk-hub-cli.exe', 'setup.exe')) {
    Copy-Item -LiteralPath (Join-Path $binDir $name) -Destination $package -Force
}
Copy-Item -LiteralPath (Join-Path $root 'README.md') -Destination $package -Force
Copy-Item -LiteralPath (Join-Path $root 'PORTING.md') -Destination $package -Force

$zip = Join-Path $root 'atk-hub-rs-portable.zip'
if (Test-Path $zip) { Remove-Item -LiteralPath $zip -Force }
Compress-Archive -Path (Join-Path $package '*') -DestinationPath $zip -CompressionLevel Optimal

Write-Host "package: $package"
Write-Host "zip:     $zip"
Get-ChildItem -LiteralPath $package | Select-Object Name, Length | Format-Table -AutoSize
