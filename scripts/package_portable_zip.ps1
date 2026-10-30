# Packages AetherIDE into portable ZIP distribution
$ErrorActionPreference = "Stop"
$IDE_ROOT = (Get-Item $PSScriptRoot).Parent.FullName
$DIST_DIR = Join-Path $IDE_ROOT "dist\windows-x64"
$ZIP_PATH = Join-Path $IDE_ROOT "dist\aether-ide-windows-x64-v0.1.0-alpha.zip"

if (-not (Test-Path $DIST_DIR)) {
    Write-Error "Please run build_windows_release.ps1 first."
}

Write-Host "Compressing AetherIDE to $ZIP_PATH..." -ForegroundColor Cyan
if (Test-Path $ZIP_PATH) {
    Remove-Item -Force $ZIP_PATH
}

Compress-Archive -Path "$DIST_DIR\*" -DestinationPath $ZIP_PATH -CompressionLevel Optimal
$hash = (Get-FileHash -Path $ZIP_PATH -Algorithm SHA256).Hash

Write-Host "Portable package created: $ZIP_PATH" -ForegroundColor Green
Write-Host "SHA256: $hash" -ForegroundColor Yellow
Set-Content -Path "$ZIP_PATH.sha256" -Value "$hash  aether-ide-windows-x64-v0.1.0-alpha.zip"
