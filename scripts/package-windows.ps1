# AetherIDE Windows Packaging Script
# Produces production-grade standalone portable distribution and Inno Setup installer

param (
    [string]$Version = "0.1.0-beta.2",
    [string]$OutputDir = "",
    [string]$TargetDir = "",
    [ValidateSet("portable", "installer", "all")]
    [string]$PackageType = "all",
    [switch]$RequireInstaller,
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
$INSTALLER_EXE = Join-Path $OutputDir "AetherIDE-Setup-$Version-x64.exe"

$PackagingResults = [System.Collections.Generic.List[PSObject]]::new()

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host " Packaging AetherIDE Windows x64 Release ($Version)" -ForegroundColor Cyan
Write-Host " Package Scope: $PackageType" -ForegroundColor Cyan
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
if ($PackageType -in @("portable", "all")) {
    Write-Host "`n[4/5] Compressing Portable ZIP Package..." -ForegroundColor Yellow
    if (Test-Path $ZIP_PACKAGE) {
        Remove-Item -Force $ZIP_PACKAGE
    }
    Compress-Archive -Path "$STAGE_DIR\*" -DestinationPath $ZIP_PACKAGE -CompressionLevel Optimal
    
    if (Test-Path $ZIP_PACKAGE) {
        $zipHash = (Get-FileHash -Path $ZIP_PACKAGE -Algorithm SHA256).Hash
        Set-Content -Path "$ZIP_PACKAGE.sha256" -Value "$zipHash  $(Split-Path $ZIP_PACKAGE -Leaf)"
        Write-Host "Created Portable Package: $ZIP_PACKAGE" -ForegroundColor Green
        Write-Host "SHA256: $zipHash" -ForegroundColor Yellow
        $PackagingResults.Add([PSCustomObject]@{
            Format = "Portable ZIP"
            Path = $ZIP_PACKAGE
            Status = "BUILT"
            Details = "Size: $((Get-Item $ZIP_PACKAGE).Length) bytes, SHA256: $zipHash"
        })
    } else {
        $PackagingResults.Add([PSCustomObject]@{
            Format = "Portable ZIP"
            Path = $ZIP_PACKAGE
            Status = "FAILED"
            Details = "Archive file not created after Compress-Archive"
        })
    }
} else {
    $PackagingResults.Add([PSCustomObject]@{
        Format = "Portable ZIP"
        Path = $ZIP_PACKAGE
        Status = "NOT_REQUESTED"
        Details = "Skipped by PackageType parameter"
    })
}

# 5. Check for Inno Setup compiler to create setup installer
if ($PackageType -in @("installer", "all")) {
    Write-Host "`n[5/5] Checking Inno Setup Compiler..." -ForegroundColor Yellow
    $isccCandidates = @(
        "iscc",
        (Join-Path $env:LOCALAPPDATA "Programs\Inno Setup 6\ISCC.exe"),
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
        if (Test-Path $INSTALLER_EXE) {
            $installerHash = (Get-FileHash -Path $INSTALLER_EXE -Algorithm SHA256).Hash
            Set-Content -Path "$INSTALLER_EXE.sha256" -Value "$installerHash  $(Split-Path $INSTALLER_EXE -Leaf)"
            Write-Host "Created Windows Setup Installer: $INSTALLER_EXE" -ForegroundColor Green
            Write-Host "SHA256: $installerHash" -ForegroundColor Yellow
            $PackagingResults.Add([PSCustomObject]@{
                Format = "Inno Setup EXE"
                Path = $INSTALLER_EXE
                Status = "BUILT"
                Details = "Size: $((Get-Item $INSTALLER_EXE).Length) bytes, SHA256: $installerHash"
            })
        } else {
            $PackagingResults.Add([PSCustomObject]@{
                Format = "Inno Setup EXE"
                Path = $INSTALLER_EXE
                Status = "FAILED"
                Details = "ISCC completed but output file not found at expected path"
            })
        }
    } else {
        $msg = "ISCC compiler not found on PATH or standard Program Files locations."
        if ($RequireInstaller) {
            throw "Installer build failed: $msg (RequireInstaller specified)"
        }
        Write-Warning "Inno Setup compiler (ISCC) not found. Installer package blocked."
        $PackagingResults.Add([PSCustomObject]@{
            Format = "Inno Setup EXE"
            Path = $INSTALLER_EXE
            Status = "BLOCKED"
            Details = $msg
        })
    }
} else {
    $PackagingResults.Add([PSCustomObject]@{
        Format = "Inno Setup EXE"
        Path = $INSTALLER_EXE
        Status = "NOT_REQUESTED"
        Details = "Skipped by PackageType parameter"
    })
}

# 6. Explicit Status Summary Matrix
Write-Host "`n============================================================" -ForegroundColor Cyan
Write-Host " Windows Release Packaging Status Summary" -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan
$PackagingResults | Format-Table -AutoSize | Out-String | Write-Host

$hasFailures = $PackagingResults | Where-Object { $_.Status -eq "FAILED" }
if ($hasFailures) {
    throw "Packaging completed with failures in required targets."
}

