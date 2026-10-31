# AetherIDE Windows Packaging Script
# Produces production-grade standalone portable distribution and Inno Setup installer

param (
    [string]$Version = "0.1.0-beta.2",
    [string]$OutputDir = "",
    [string]$TargetDir = "",
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$IDE_ROOT = (Get-Item $PSScriptRoot).Parent.FullName

if (-not $OutputDir) {
    $OutputDir = Join-Path $IDE_ROOT "dist"
}

if (-not $TargetDir) {
    if ($env:CARGO_TARGET_DIR) {
        $TargetDir = $env:CARGO_TARGET_DIR
    } else {
        $TargetDir = Join-Path $env:LOCALAPPDATA "AetherIDE\build-cache\aetheride\windows-x64"
    }
}

$RELEASE_BIN_DIR = Join-Path $TargetDir "release"
$STAGE_DIR = Join-Path $OutputDir "windows-x64"
$ZIP_PACKAGE = Join-Path $OutputDir "AetherIDE-v$Version-windows-x64.zip"

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host " Packaging AetherIDE Windows x64 Release ($Version)" -ForegroundColor Cyan
Write-Host " Output Directory: $OutputDir" -ForegroundColor Cyan
Write-Host " Target Directory: $TargetDir" -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan

# 1. Optionally compile release binaries
if (-not $SkipBuild) {
    Write-Host "`n[1/5] Compiling Release Binaries with external cache..." -ForegroundColor Yellow
    $env:CARGO_TARGET_DIR = $TargetDir
    Set-Location "$IDE_ROOT\rust"
    cargo build --release --bin aether-daemon --bin aether-cli --bin AetherIDE

    Write-Host "[2/5] Building Desktop Workbench..." -ForegroundColor Yellow
    Set-Location "$IDE_ROOT\apps\desktop"
    npm run build
}

# 2. Verify native binaries exist
$AETHER_IDE = Join-Path $RELEASE_BIN_DIR "AetherIDE.exe"
$AETHER_DAEMON = Join-Path $RELEASE_BIN_DIR "aether-daemon.exe"
$AETHER_CLI = Join-Path $RELEASE_BIN_DIR "aether-cli.exe"

foreach ($bin in @($AETHER_IDE, $AETHER_DAEMON, $AETHER_CLI)) {
    if (-not (Test-Path $bin)) {
        throw "Required binary not found: $bin. Please run cargo build --release first."
    }
}

# 3. Clean and prepare staging directory
Write-Host "`n[3/5] Assembling Staging Layout..." -ForegroundColor Yellow
if (Test-Path $STAGE_DIR) {
    Remove-Item -Recurse -Force $STAGE_DIR
}
New-Item -ItemType Directory -Path $STAGE_DIR -Force | Out-Null
New-Item -ItemType Directory -Path "$STAGE_DIR\bin" -Force | Out-Null
New-Item -ItemType Directory -Path "$STAGE_DIR\workbench" -Force | Out-Null

Copy-Item $AETHER_IDE "$STAGE_DIR\AetherIDE.exe" -Force
Copy-Item $AETHER_DAEMON "$STAGE_DIR\bin\aether-daemon.exe" -Force
Copy-Item $AETHER_CLI "$STAGE_DIR\bin\aether-cli.exe" -Force

# Copy workbench files
if (Test-Path "$IDE_ROOT\apps\desktop\workbench") {
    Copy-Item "$IDE_ROOT\apps\desktop\workbench\*" "$STAGE_DIR\workbench\" -Recurse -Force
}
if (Test-Path "$IDE_ROOT\apps\desktop\dist") {
    Copy-Item "$IDE_ROOT\apps\desktop\dist" "$STAGE_DIR\workbench\" -Recurse -Force
}
if (Test-Path "$IDE_ROOT\apps\desktop\package.json") {
    Copy-Item "$IDE_ROOT\apps\desktop\package.json" "$STAGE_DIR\workbench\" -Force
}

# Copy documentation and licenses
Copy-Item "$IDE_ROOT\LICENSE" "$STAGE_DIR\LICENSE" -Force
Copy-Item "$IDE_ROOT\README.md" "$STAGE_DIR\README.md" -Force

# Launcher batch helper
$LAUNCHER_BAT = @"
@echo off
setlocal
set "AETHER_ROOT=%~dp0"
set "PATH=%AETHER_ROOT%bin;%PATH%"
start "" "%AETHER_ROOT%AetherIDE.exe" %*
endlocal
"@
Set-Content -Path "$STAGE_DIR\AetherIDE.cmd" -Value $LAUNCHER_BAT

# Metadata descriptor
$RELEASE_INFO = @{
    name = "AetherIDE"
    version = $Version
    platform = "windows-x64"
    channel = "beta"
    buildDate = (Get-Date -Format "yyyy-MM-ddTHH:mm:ssZ")
    binaries = @("AetherIDE.exe", "bin/aether-daemon.exe", "bin/aether-cli.exe")
} | ConvertTo-Json -Depth 4
Set-Content -Path "$STAGE_DIR\RELEASE_INFO.json" -Value $RELEASE_INFO
Set-Content -Path "$STAGE_DIR\VERSION.txt" -Value $Version

# 4. Generate Portable ZIP Distribution
Write-Host "`n[4/5] Compressing Portable ZIP Package..." -ForegroundColor Yellow
if (Test-Path $ZIP_PACKAGE) {
    Remove-Item -Force $ZIP_PACKAGE
}
Compress-Archive -Path "$STAGE_DIR\*" -DestinationPath $ZIP_PACKAGE -CompressionLevel Optimal
$zipHash = (Get-FileHash -Path $ZIP_PACKAGE -Algorithm SHA256).Hash
Set-Content -Path "$ZIP_PACKAGE.sha256" -Value "$zipHash  $(Split-Path $ZIP_PACKAGE -Leaf)"
Write-Host "Created Portable Package: $ZIP_PACKAGE" -ForegroundColor Green
Write-Host "SHA256: $zipHash" -ForegroundColor Yellow

# 5. Check for Inno Setup compiler to create setup installer
Write-Host "`n[5/5] Checking Inno Setup Compiler..." -ForegroundColor Yellow
$isccCandidates = @(
    "iscc",
    "C:\Program Files (x86)\Inno Setup 6\ISCC.exe",
    "C:\Program Files\Inno Setup 6\ISCC.exe"
)
$isccPath = $null
foreach ($c in $isccCandidates) {
    if (Get-Command $c -ErrorAction SilentlyContinue) {
        $isccPath = $c
        break
    } elseif (Test-Path $c) {
        $isccPath = $c
        break
    }
}

if ($isccPath) {
    Write-Host "Compiling Inno Setup installer using $isccPath..." -ForegroundColor Green
    & $isccPath "$IDE_ROOT\scripts\installer\aetheride_installer.iss"
    $installerExe = Join-Path $OutputDir "AetherIDE-v$Version-windows-x64-setup.exe"
    if (Test-Path $installerExe) {
        $installerHash = (Get-FileHash -Path $installerExe -Algorithm SHA256).Hash
        Set-Content -Path "$installerExe.sha256" -Value "$installerHash  $(Split-Path $installerExe -Leaf)"
        Write-Host "Created Windows Setup Installer: $installerExe" -ForegroundColor Green
        Write-Host "SHA256: $installerHash" -ForegroundColor Yellow
    }
} else {
    Write-Host "ISCC not detected in current environment. Standalone portable package is ready." -ForegroundColor Cyan
    Write-Host "Installer script available at: scripts\installer\aetheride_installer.iss" -ForegroundColor Cyan
}

Write-Host "`n============================================================" -ForegroundColor Green
Write-Host " Windows Release Packaging Complete!" -ForegroundColor Green
Write-Host "============================================================" -ForegroundColor Green
