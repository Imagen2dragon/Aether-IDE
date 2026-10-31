# AetherIDE Build Orchestration Script (PowerShell / Windows)
# Enforces external build cache location outside OneDrive

param(
    [Parameter(Position=0)]
    [ValidateSet("dev", "check", "test", "release", "windows", "linux", "macos", "inspect-target")]
    [string]$Command = "check",

    [switch]$VerboseOutput,
    [string]$TargetTriple = ""
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir
$RustDir = Join-Path $ProjectRoot "rust"

# 1. Resolve External Build Cache
$ExternalCacheBase = "$env:LOCALAPPDATA\AetherIDE\build-cache"
if (-not (Test-Path $ExternalCacheBase)) {
    New-Item -ItemType Directory -Path "$ExternalCacheBase\aetheride\windows-x64" -Force | Out-Null
    New-Item -ItemType Directory -Path "$ExternalCacheBase\aetheride\linux-x64" -Force | Out-Null
    New-Item -ItemType Directory -Path "$ExternalCacheBase\aetheride\macos-arm64" -Force | Out-Null
}

$DefaultTargetDir = "$ExternalCacheBase\aetheride\windows-x64"

# If CARGO_TARGET_DIR is not set, point to external cache
if (-not $env:CARGO_TARGET_DIR) {
    $env:CARGO_TARGET_DIR = $DefaultTargetDir
}

# 2. Check sccache
if (Get-Command sccache -ErrorAction SilentlyContinue) {
    if (-not $env:RUSTC_WRAPPER) {
        $env:RUSTC_WRAPPER = "sccache"
        $env:SCCACHE_DIR = "$ExternalCacheBase\sccache"
        Write-Host "[AetherBuild] Compiler cache sccache enabled (cache: $env:SCCACHE_DIR)" -ForegroundColor Cyan
    }
}

# 3. Handle Commands
Write-Host "[AetherBuild] Target Directory: $env:CARGO_TARGET_DIR" -ForegroundColor DarkGray

switch ($Command) {
    "inspect-target" {
        Write-Host "Active CARGO_TARGET_DIR: $env:CARGO_TARGET_DIR" -ForegroundColor Green
        if (Test-Path $env:CARGO_TARGET_DIR) {
            $size = (Get-ChildItem $env:CARGO_TARGET_DIR -Recurse -File -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum
            Write-Host "Current Cache Footprint: $([math]::Round($size / 1MB, 2)) MB ($([math]::Round($size / 1GB, 3)) GB)" -ForegroundColor Green
        } else {
            Write-Host "Target directory not yet populated." -ForegroundColor Yellow
        }
        return
    }

    "check" {
        Write-Host "[AetherBuild] Running cargo check across workspace..." -ForegroundColor Cyan
        Set-Location $RustDir
        cargo check --workspace --all-targets
    }

    "dev" {
        Write-Host "[AetherBuild] Building workspace (development profile)..." -ForegroundColor Cyan
        Set-Location $RustDir
        cargo build --workspace --all-targets
    }

    "test" {
        Write-Host "[AetherBuild] Running test suite across workspace..." -ForegroundColor Cyan
        Set-Location $RustDir
        cargo test --workspace --all-targets
    }

    "release" {
        Write-Host "[AetherBuild] Building workspace (release profile)..." -ForegroundColor Cyan
        Set-Location $RustDir
        cargo build --workspace --release
    }

    "windows" {
        Write-Host "[AetherBuild] Compiling Windows Native Release Binaries..." -ForegroundColor Green
        Set-Location $RustDir
        cargo build --release -p aether-launcher -p aether-daemon -p aether-cli
        
        $DistBin = Join-Path $ProjectRoot "dist\windows-x64\bin"
        $DistRoot = Join-Path $ProjectRoot "dist\windows-x64"
        if (-not (Test-Path $DistBin)) { New-Item -ItemType Directory -Path $DistBin -Force | Out-Null }

        Copy-Item -Force "$env:CARGO_TARGET_DIR\release\AetherIDE.exe" "$DistRoot\AetherIDE.exe"
        Copy-Item -Force "$env:CARGO_TARGET_DIR\release\aether-daemon.exe" "$DistBin\aether-daemon.exe"
        Copy-Item -Force "$env:CARGO_TARGET_DIR\release\aether-cli.exe" "$DistBin\aether-cli.exe"
        Write-Host "[AetherBuild] Windows binaries updated in $DistRoot" -ForegroundColor Green
    }

    "linux" {
        Write-Host "[AetherBuild] Target Linux (x86_64-unknown-linux-gnu)..." -ForegroundColor Cyan
        Write-Host "Note: Native GUI linking requires Linux host / CI runner with libwebkit2gtk-4.1-dev." -ForegroundColor Yellow
        Set-Location $RustDir
        cargo check --target x86_64-unknown-linux-gnu -p aether-daemon -p aether-cli
    }

    "macos" {
        Write-Host "[AetherBuild] Target macOS (aarch64-apple-darwin)..." -ForegroundColor Cyan
        Write-Host "Note: Native GUI linking requires macOS host / CI runner with Xcode SDK." -ForegroundColor Yellow
        Set-Location $RustDir
        cargo check --target aarch64-apple-darwin -p aether-daemon -p aether-cli
    }
}
