# Generates SHA256SUMS.txt and RELEASE_MANIFEST.json for all release artifacts
param (
    [string]$DistDir = ""
)

$ErrorActionPreference = "Stop"
$IDE_ROOT = (Get-Item $PSScriptRoot).Parent.FullName

if (-not $DistDir) {
    $DistDir = Join-Path $IDE_ROOT "dist"
}

if (-not (Test-Path $DistDir)) {
    throw "Distribution directory not found: $DistDir"
}

Write-Host "Scanning release packages in: $DistDir" -ForegroundColor Cyan

$packageExtensions = @(".zip", ".tar.gz", ".deb", ".AppImage", ".dmg", ".exe")
$artifacts = Get-ChildItem -Path $DistDir -File | Where-Object {
    $ext = $_.Extension
    $name = $_.Name
    ($packageExtensions -contains $ext -or $name.EndsWith(".tar.gz")) -and
    -not $name.EndsWith(".sha256") -and
    $name -ne "SHA256SUMS.txt"
}

if ($artifacts.Count -eq 0) {
    Write-Warning "No release packages found in $DistDir"
}

$checksumLines = @()
$manifestEntries = @()

foreach ($art in $artifacts) {
    $hash = (Get-FileHash -Path $art.FullName -Algorithm SHA256).Hash.ToLower()
    $size = $art.Length
    $name = $art.Name
    
    $checksumLines += "$hash  $name"

    $platform = "unknown"
    if ($name -match "windows|win32|\.exe$|\.zip$") {
        $platform = "windows-x64"
    } elseif ($name -match "linux|\.deb$|\.AppImage$") {
        $platform = "linux-x64"
    } elseif ($name -match "macos|darwin|\.dmg$") {
        $platform = "macos-arm64"
    }

    $manifestEntries += @{
        filename = $name
        platform = $platform
        size_bytes = $size
        sha256 = $hash
    }
}

$shaFile = Join-Path $DistDir "SHA256SUMS.txt"
Set-Content -Path $shaFile -Value $checksumLines -Encoding utf8
Write-Host "Wrote checksum manifest: $shaFile" -ForegroundColor Green

$manifest = @{
    release = "v0.1.0-beta.2"
    generated_at = (Get-Date -Format "yyyy-MM-ddTHH:mm:ssZ")
    artifacts = $manifestEntries
}

$jsonFile = Join-Path $DistDir "RELEASE_MANIFEST.json"
$manifest | ConvertTo-Json -Depth 4 | Set-Content -Path $jsonFile -Encoding utf8
Write-Host "Wrote release metadata manifest: $jsonFile" -ForegroundColor Green
