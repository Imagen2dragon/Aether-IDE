# Copyright (c) Microsoft Corporation. All rights reserved.
# Licensed under the MIT License. See License.txt in the project root for license information.

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$packageRoot = Join-Path (Split-Path $repoRoot -Parent) 'VSCode-win32-x64'
$productPath = Join-Path $packageRoot 'resources/app/product.json'
$sourceExe = Join-Path $packageRoot 'AetherIDE Preview.exe'
if (-not (Test-Path -LiteralPath $productPath) -or -not (Test-Path -LiteralPath $sourceExe)) {
	throw 'Build the Windows package with dev.ps1 -Task Package before installing.'
}
$product = Get-Content -LiteralPath $productPath -Raw | ConvertFrom-Json
if ($product.nameShort -ne 'AetherIDE Preview' -or $product.win32AppUserModelId -ne 'AetherIDE.CodeOSSPreview') {
	throw 'The package does not have the expected AetherIDE Preview identity.'
}
if ($product.commit -notmatch '^[a-f0-9]{40}$') { throw 'The package has no valid source commit.' }
$expectedCommit = (& git -C $repoRoot rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $product.commit -ne $expectedCommit) {
	throw 'The package source commit does not match this checkout. Rebuild before installing.'
}

# Each install creates a new release. Existing releases and legacy AetherIDE
# installations are never overwritten; the shortcuts switch after verification.
$installRoot = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'Programs/AetherIDE Preview'
$releaseName = $product.commit.Substring(0, 12) + '-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '-' + [guid]::NewGuid().ToString('N').Substring(0, 6)
$releaseRoot = [IO.Path]::GetFullPath((Join-Path $installRoot "releases/$releaseName"))
$installPrefix = [IO.Path]::GetFullPath($installRoot).TrimEnd('\') + '\'
if (-not $releaseRoot.StartsWith($installPrefix, [StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $releaseRoot)) {
	throw 'The preview release destination is not a new directory within the installation root.'
}
New-Item -ItemType Directory -Path $releaseRoot -Force | Out-Null
Get-ChildItem -LiteralPath $packageRoot -Force | Copy-Item -Destination $releaseRoot -Recurse
$installedExe = Join-Path $releaseRoot 'AetherIDE Preview.exe'
$installedProduct = Join-Path $releaseRoot 'resources/app/product.json'
foreach ($pair in @(@($sourceExe, $installedExe), @($productPath, $installedProduct))) {
	if ((Get-FileHash -LiteralPath $pair[0] -Algorithm SHA256).Hash -ne (Get-FileHash -LiteralPath $pair[1] -Algorithm SHA256).Hash) {
		throw 'The installed preview did not match the package. Existing shortcuts were not changed.'
	}
}

$manifestPath = Join-Path $installRoot 'current.json'
$previousRelease = $null
if (Test-Path -LiteralPath $manifestPath) {
	$previousRelease = (Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json).releaseDirectory
}
$shortcutWriter = New-Object -ComObject WScript.Shell
foreach ($shortcutRoot in @([Environment]::GetFolderPath('Programs'), [Environment]::GetFolderPath('DesktopDirectory'))) {
	$shortcut = $shortcutWriter.CreateShortcut((Join-Path $shortcutRoot 'AetherIDE Preview.lnk'))
	$shortcut.TargetPath = $installedExe
	$shortcut.WorkingDirectory = $releaseRoot
	$shortcut.IconLocation = $installedExe + ',0'
	$shortcut.Description = 'AetherIDE desktop preview built on Code - OSS'
	$shortcut.Save()
}
$manifest = [ordered]@{
	sourceCommit = $product.commit
	version = $product.version
	releaseDirectory = $releaseRoot
	executable = $installedExe
	previousReleaseDirectory = $previousRelease
	installedAtUtc = [DateTime]::UtcNow.ToString('o')
	channel = 'local-preview'
}
[IO.File]::WriteAllText($manifestPath, ($manifest | ConvertTo-Json) + "`n", [Text.UTF8Encoding]::new($false))
Write-Output "Installed AetherIDE Preview: $installedExe"
Write-Output "Source commit: $($product.commit)"
if ($previousRelease) { Write-Output "Previous release retained: $previousRelease" }
