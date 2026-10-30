# AetherIDE Windows Release Build & Package Script
# Builds high-performance native Rust core + TypeScript workbench

$ErrorActionPreference = "Stop"
$IDE_ROOT = (Get-Item $PSScriptRoot).Parent.FullName
$DIST_DIR = Join-Path $IDE_ROOT "dist\windows-x64"

Write-Host "==========================================" -ForegroundColor Cyan
Write-Host " Building AetherIDE Windows x64 Production" -ForegroundColor Cyan
Write-Host "==========================================" -ForegroundColor Cyan

# 1. Clean previous build artifacts
if (Test-Path $DIST_DIR) {
    Remove-Item -Recurse -Force $DIST_DIR
}
New-Item -ItemType Directory -Path $DIST_DIR -Force | Out-Null
New-Item -ItemType Directory -Path "$DIST_DIR\bin" -Force | Out-Null
New-Item -ItemType Directory -Path "$DIST_DIR\extensions" -Force | Out-Null

# 2. Build Rust native core binaries in release mode
Write-Host "`n[1/4] Compiling Native Rust Daemon and CLI..." -ForegroundColor Yellow
Set-Location "$IDE_ROOT\rust"
cargo build --release --bin aether-daemon --bin aether-cli

# Copy Rust binaries
Copy-Item "$IDE_ROOT\rust\target\release\aether-daemon.exe" "$DIST_DIR\bin\" -Force
Copy-Item "$IDE_ROOT\rust\target\release\aether-cli.exe" "$DIST_DIR\bin\" -Force
Write-Host "Copied native binaries: aether-daemon.exe, aether-cli.exe" -ForegroundColor Green

# 3. Compile Aether Core Extension
Write-Host "`n[2/4] Building Aether Core Extension..." -ForegroundColor Yellow
Set-Location "$IDE_ROOT\extensions\aether-core"
if (Test-Path "package.json") {
    # Ensure dependencies compiled
    npx tsc -p ./
    Copy-Item "$IDE_ROOT\extensions\aether-core" "$DIST_DIR\extensions\" -Recurse -Force
}

# 4. Generate Launcher & Portable Distribution
Write-Host "`n[3/4] Generating Launcher and Start Scripts..." -ForegroundColor Yellow
$LAUNCHER_BAT = @"
@echo off
title AetherIDE - Autonomous AI Engineering Platform
echo Starting Aether Native Daemon...
start /B "" "%~dp0bin\aether-daemon.exe"
timeout /t 1 /nobreak >nul
echo Launching AetherIDE Desktop Workbench...
start "" code "%~dp0.." --extensionDevelopmentPath="%~dp0extensions\aether-core"
exit
"@
Set-Content -Path "$DIST_DIR\AetherIDE.cmd" -Value $LAUNCHER_BAT

# 5. Create Checksums
Write-Host "`n[4/4] Generating SHA256 Verification Checksums..." -ForegroundColor Yellow
Set-Location $DIST_DIR
$files = Get-ChildItem -Recurse -File
$checksums = foreach ($file in $files) {
    $hash = (Get-FileHash -Path $file.FullName -Algorithm SHA256).Hash
    "$hash  $($file.FullName.Replace($DIST_DIR, ''))"
}
Set-Content -Path "$DIST_DIR\CHECKSUMS-SHA256.txt" -Value $checksums

Write-Host "`n==========================================" -ForegroundColor Green
Write-Host " AetherIDE Windows x64 Build Complete!" -ForegroundColor Green
Write-Host " Location: $DIST_DIR" -ForegroundColor Green
Write-Host "==========================================" -ForegroundColor Green
