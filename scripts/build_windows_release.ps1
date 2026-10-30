# AetherIDE Windows Release Build & Package Script
# Builds high-performance native Rust core + Standalone Desktop Workbench

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
New-Item -ItemType Directory -Path "$DIST_DIR\extensions\aether-core" -Force | Out-Null
New-Item -ItemType Directory -Path "$DIST_DIR\workbench\dist" -Force | Out-Null

# 2. Build Rust native core binaries in release mode
Write-Host "`n[1/4] Compiling Native Rust Daemon, CLI and Standalone Launcher..." -ForegroundColor Yellow
Set-Location "$IDE_ROOT\rust"
cargo build --release --bin aether-daemon --bin aether-cli --bin AetherIDE

# Copy Rust binaries
Copy-Item "$IDE_ROOT\rust\target\release\aether-daemon.exe" "$DIST_DIR\bin\" -Force
Copy-Item "$IDE_ROOT\rust\target\release\aether-cli.exe" "$DIST_DIR\bin\" -Force
Copy-Item "$IDE_ROOT\rust\target\release\AetherIDE.exe" "$DIST_DIR\AetherIDE.exe" -Force
Write-Host "Copied native binaries: AetherIDE.exe, bin\aether-daemon.exe, bin\aether-cli.exe" -ForegroundColor Green

# 3. Compile Aether Core Extension
Write-Host "`n[2/4] Building Aether Core Extension..." -ForegroundColor Yellow
Set-Location "$IDE_ROOT\extensions\aether-core"
$tscPath = "$IDE_ROOT\extensions\aether-core\node_modules\typescript\bin\tsc"
if (Test-Path $tscPath) {
    node $tscPath -p "$IDE_ROOT\extensions\aether-core"
    Copy-Item "$IDE_ROOT\extensions\aether-core\dist" "$DIST_DIR\extensions\aether-core\dist" -Recurse -Force
    Copy-Item "$IDE_ROOT\extensions\aether-core\package.json" "$DIST_DIR\extensions\aether-core\" -Force
    Write-Host "Compiled and packaged Aether Core Extension" -ForegroundColor Green
} else {
    Write-Warning "TypeScript compiler not found in local extension node_modules."
}

# 4. Compile Desktop Workbench
Write-Host "`n[3/4] Building Standalone Desktop Workbench..." -ForegroundColor Yellow
Set-Location "$IDE_ROOT\apps\desktop"
if (Test-Path $tscPath) {
    node $tscPath -p "$IDE_ROOT\apps\desktop"
}
Copy-Item "$IDE_ROOT\apps\desktop\dist\*" "$DIST_DIR\workbench\dist\" -Recurse -Force
Copy-Item "$IDE_ROOT\apps\desktop\workbench\index.html" "$DIST_DIR\workbench\index.html" -Force
Copy-Item "$IDE_ROOT\apps\desktop\package.json" "$DIST_DIR\workbench\" -Force
Write-Host "Packaged Standalone Desktop Workbench" -ForegroundColor Green

# 5. Generate Standalone Launcher Batch
$LAUNCHER_BAT = @"
@echo off
title AetherIDE - Autonomous AI Engineering Platform
"%~dp0AetherIDE.exe" %*
"@
Set-Content -Path "$DIST_DIR\AetherIDE.cmd" -Value $LAUNCHER_BAT

# 6. Create Checksums
Write-Host "`n[4/4] Generating SHA256 Verification Checksums..." -ForegroundColor Yellow
Set-Location $DIST_DIR
$files = Get-ChildItem -Recurse -File | Where-Object { $_.Name -ne "CHECKSUMS-SHA256.txt" }
$checksums = foreach ($file in $files) {
    $hash = (Get-FileHash -Path $file.FullName -Algorithm SHA256).Hash
    "$hash  $($file.FullName.Replace($DIST_DIR, ''))"
}
Set-Content -Path "$DIST_DIR\CHECKSUMS-SHA256.txt" -Value $checksums

Write-Host "`n==========================================" -ForegroundColor Green
Write-Host " AetherIDE Windows x64 Build Complete!" -ForegroundColor Green
Write-Host " Location: $DIST_DIR" -ForegroundColor Green
Write-Host "==========================================" -ForegroundColor Green
