# packaging/windows/package-msix.ps1
# Automated Windows MSIX Packaging Pipeline for AetherIDE
param(
    [string]$DistDir = "$PSScriptRoot\..\..\dist\windows-x64",
    [string]$OutputDir = "$PSScriptRoot\..\..\dist\installers",
    [string]$CertificatePath = "",
    [string]$CertificatePassword = ""
)

$ErrorActionPreference = "Stop"

Write-Host "============================================================" -ForegroundColor Cyan
Write-Host " AetherIDE — MSIX Enterprise Packaging Pipeline" -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan

if (-not (Test-Path $DistDir)) {
    Write-Error "Distribution directory not found at $DistDir. Run scripts/build.ps1 first."
}

if (-not (Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
}

$LayoutDir = Join-Path $env:TEMP "AetherIDE_MSIX_Layout"
if (Test-Path $LayoutDir) {
    Remove-Item -Path $LayoutDir -Recurse -Force
}
New-Item -ItemType Directory -Path $LayoutDir -Force | Out-Null

Write-Host " [*] Staging payload to temporary layout directory..."
Copy-Item -Path "$DistDir\*" -Destination $LayoutDir -Recurse -Force
Copy-Item -Path "$PSScriptRoot\AppxManifest.xml" -Destination "$LayoutDir\AppxManifest.xml" -Force

$AssetsDir = Join-Path $LayoutDir "Assets"
if (-not (Test-Path $AssetsDir)) {
    New-Item -ItemType Directory -Path $AssetsDir -Force | Out-Null
    # Create placeholder visual asset bitmaps if not present
    1..3 | ForEach-Object {
        $bmpPath = switch ($_) {
            1 { "$AssetsDir\StoreLogo.png" }
            2 { "$AssetsDir\Square150x150Logo.png" }
            3 { "$AssetsDir\Square44x44Logo.png" }
        }
        if (-not (Test-Path $bmpPath)) {
            [System.IO.File]::WriteAllBytes($bmpPath, [byte[]]@(0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A))
        }
    }
}

$MsixPath = Join-Path $OutputDir "AetherIDE-v0.1.0-alpha-x64.msix"

# Look for MakeAppx.exe in Windows Kits
$WindowsKitsPath = "C:\Program Files (x86)\Windows Kits\10\bin"
$MakeAppx = Get-ChildItem -Path $WindowsKitsPath -Filter "MakeAppx.exe" -Recurse -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -match "x64" } |
    Select-Object -First 1 -ExpandProperty FullName

if ($MakeAppx -and (Test-Path $MakeAppx)) {
    Write-Host " [✓] Found Windows SDK MakeAppx: $MakeAppx"
    Write-Host " [*] Packaging MSIX container..."
    & $MakeAppx pack /d $LayoutDir /p $MsixPath /o
    Write-Host " [✓] MSIX package generated successfully: $MsixPath" -ForegroundColor Green
} else {
    Write-Warning "MakeAppx.exe not found in Windows Kits. Staged MSIX layout is ready at $LayoutDir for manual packaging."
}

Write-Host " [✓] Packaging workflow complete." -ForegroundColor Green
