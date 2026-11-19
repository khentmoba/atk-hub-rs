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
Copy-Item -LiteralPath (Join-Path (Split-Path -Parent $root) 'assets\atk-hub-logo.png') -Destination $package -Force
Copy-Item -LiteralPath (Join-Path (Split-Path -Parent $root) 'assets\atk-hub.ico') -Destination $package -Force
Copy-Item -LiteralPath (Join-Path $root 'README.md') -Destination $package -Force
Copy-Item -LiteralPath (Join-Path $root 'PORTING.md') -Destination $package -Force

$zip = Join-Path $root 'atk-hub-rs-portable.zip'
if (Test-Path $zip) { Remove-Item -LiteralPath $zip -Force }
Compress-Archive -Path (Join-Path $package '*') -DestinationPath $zip -CompressionLevel Optimal

$stub = Join-Path $binDir 'setup.exe'
$stubBytes = [System.IO.File]::ReadAllBytes($stub)
$zipBytes = [System.IO.File]::ReadAllBytes($zip)
$marker = [System.Text.Encoding]::ASCII.GetBytes("ATK-SELF-EXTRACT-1`n")
$combined = New-Object byte[] ($stubBytes.Length + $marker.Length + $zipBytes.Length)
[Array]::Copy($stubBytes, 0, $combined, 0, $stubBytes.Length)
[Array]::Copy($marker, 0, $combined, $stubBytes.Length, $marker.Length)
[Array]::Copy($zipBytes, 0, $combined, $stubBytes.Length + $marker.Length, $zipBytes.Length)
$single = Join-Path $root 'setup.exe'
[System.IO.File]::WriteAllBytes($single, $combined)

Write-Host "package: $package"
Write-Host "zip:     $zip"
Write-Host "single:  $single"
Get-ChildItem -LiteralPath $package | Select-Object Name, Length | Format-Table -AutoSize
