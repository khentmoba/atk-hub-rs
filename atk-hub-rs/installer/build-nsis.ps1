# Builds the NSIS setup.exe used by the SlickShot-style installer.
param(
    [string]$Version = '0.1.5'
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$staging = Join-Path $root 'installer-package'
if (-not (Test-Path $staging)) {
    throw "Run build-package.ps1 first to create $staging"
}

$makensis = (Get-Command makensis -ErrorAction SilentlyContinue).Source
if (-not $makensis) {
    $candidates = @(
        'C:\Program Files (x86)\NSIS\makensis.exe',
        'C:\Program Files\NSIS\makensis.exe'
    )
    $makensis = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $makensis) { throw 'makensis not found. Install NSIS (winget install NSIS.NSIS).' }

$output = Join-Path $root 'ATK-HUB-setup.exe'
& $makensis "/DVERSION=$Version" "/DSTAGING=$staging" "/DOUTFILE=$output" (Join-Path $PSScriptRoot 'AtkHub.nsi')
if ($LASTEXITCODE -ne 0) { throw "makensis failed with exit code $LASTEXITCODE" }
Get-Item -LiteralPath $output | Select-Object FullName, Length | Format-List
