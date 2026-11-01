# AetherIDE Tauri 2 Desktop Packaging Script
# Compiles the production frontend and Tauri 2 host into a standalone desktop executable and NSIS installer.

param (
    [string]$Version = "0.1.0-beta.2",
    [string]$OutputDir = ""
)

$ErrorActionPreference = "Stop"
$IDE_ROOT = (Get-Item $PSScriptRoot).Parent.FullName

if (-not $OutputDir) {
    $OutputDir = Join-Path $IDE_ROOT "dist"
}

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host " Building AetherIDE Tauri 2 Desktop Release ($Version)" -ForegroundColor Cyan
Write-Host " Output Directory: $OutputDir" -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan

# 1. Build Desktop Workbench Frontend
Write-Host "`n[1/3] Building Workbench Frontend..." -ForegroundColor Yellow
Set-Location "$IDE_ROOT\apps\desktop"
npm run build

# 2. Build Tauri 2 Native Executable & NSIS Setup
Write-Host "`n[2/3] Building Tauri 2 Executable and NSIS Installer..." -ForegroundColor Yellow
npx tauri build

$TAURI_NSIS = "$IDE_ROOT\apps\desktop\src-tauri\target\release\bundle\nsis\AetherIDE_0.1.0_x64-setup.exe"
$TAURI_EXE = "$IDE_ROOT\apps\desktop\src-tauri\target\release\AetherIDE.exe"

if (-not (Test-Path $TAURI_NSIS) -or -not (Test-Path $TAURI_EXE)) {
    throw "Tauri build artifacts missing. Expected $TAURI_NSIS and $TAURI_EXE"
}

# 3. Assemble Distribution Artifacts
Write-Host "`n[3/3] Copying Distribution Artifacts and Computing SHA-256..." -ForegroundColor Yellow
if (-not (Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
}

$DEST_SETUP = Join-Path $OutputDir "AetherIDE-Setup-$Version-x64.exe"
$DEST_PORTABLE_DIR = Join-Path $OutputDir "windows-x64"
$DEST_ZIP = Join-Path $OutputDir "AetherIDE-v$Version-windows-x64.zip"

Copy-Item -Path $TAURI_NSIS -Destination $DEST_SETUP -Force
Copy-Item -Path $TAURI_EXE -Destination "$DEST_PORTABLE_DIR\AetherIDE.exe" -Force

if (Test-Path $DEST_ZIP) {
    Remove-Item -Path $DEST_ZIP -Force
}
Compress-Archive -Path "$DEST_PORTABLE_DIR\*" -DestinationPath $DEST_ZIP

$setupHash = (Get-FileHash -Path $DEST_SETUP -Algorithm SHA256).Hash.ToLower()
$zipHash = (Get-FileHash -Path $DEST_ZIP -Algorithm SHA256).Hash.ToLower()

"$setupHash  AetherIDE-Setup-$Version-x64.exe" | Out-File -Encoding ascii "$DEST_SETUP.sha256"
"$zipHash  AetherIDE-v$Version-windows-x64.zip" | Out-File -Encoding ascii "$DEST_ZIP.sha256"

$sums = @"
$setupHash  AetherIDE-Setup-$Version-x64.exe
$zipHash  AetherIDE-v$Version-windows-x64.zip
"@
$sums | Out-File -Encoding ascii "$OutputDir\SHA256SUMS.txt"

Write-Host "`n============================================================" -ForegroundColor Green
Write-Host " Tauri 2 Release Built Successfully!" -ForegroundColor Green
Write-Host " Setup EXE: $DEST_SETUP ($((Get-Item $DEST_SETUP).Length) bytes)" -ForegroundColor Green
Write-Host " Portable ZIP: $DEST_ZIP ($((Get-Item $DEST_ZIP).Length) bytes)" -ForegroundColor Green
Write-Host " Setup SHA256: $setupHash" -ForegroundColor Yellow
Write-Host " Portable SHA256: $zipHash" -ForegroundColor Yellow
Write-Host "============================================================" -ForegroundColor Green
