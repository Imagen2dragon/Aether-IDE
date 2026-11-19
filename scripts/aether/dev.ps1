# Copyright (c) Microsoft Corporation. All rights reserved.
# Licensed under the MIT License. See License.txt in the project root for license information.

[CmdletBinding()]
param(
	[ValidateSet('Setup', 'Install', 'Build', 'Electron', 'Launch', 'Package')]
	[string] $Task = 'Setup',
	[string] $NodeDirectory,
	[string] $PnpmDirectory,
	[switch] $SkipDictationRuntime,
	[switch] $SkipBrowserDownload,
	[string] $OpenPath
)

$ErrorActionPreference = 'Stop'
$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$toolsRoot = Join-Path $repoRoot '.build/aether-tools'
if ($NodeDirectory) { $NodeDirectory = [IO.Path]::GetFullPath($NodeDirectory) }
if ($PnpmDirectory) { $PnpmDirectory = [IO.Path]::GetFullPath($PnpmDirectory) }
$package = Get-Content -LiteralPath (Join-Path $repoRoot 'package.json') -Raw | ConvertFrom-Json
$nodeVersion = (Get-Content -LiteralPath (Join-Path $repoRoot '.nvmrc') -Raw).Trim()
$pnpmVersion = ($package.packageManager -split '@')[1]
if ([Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne 'X64') {
	throw 'This Windows bootstrap currently supports x64 only.'
}
New-Item -ItemType Directory -Path $toolsRoot -Force | Out-Null

if (-not $NodeDirectory) {
	$NodeDirectory = Join-Path $toolsRoot "node-v$nodeVersion-win-x64"
	if (-not (Test-Path -LiteralPath (Join-Path $NodeDirectory 'node.exe'))) {
		$archiveName = "node-v$nodeVersion-win-x64.zip"
		$archive = Join-Path $toolsRoot $archiveName
		$checksums = (Invoke-WebRequest -Uri "https://nodejs.org/dist/v$nodeVersion/SHASUMS256.txt").Content
		$checksumLine = $checksums -split "`n" | Where-Object { $_.TrimEnd().EndsWith("  $archiveName") }
		if (-not $checksumLine) { throw "Official checksum missing for $archiveName" }
		Invoke-WebRequest -Uri "https://nodejs.org/dist/v$nodeVersion/$archiveName" -OutFile $archive
		if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne ($checksumLine -split '\s+')[0]) {
			throw 'Node.js archive checksum mismatch.'
		}
		Expand-Archive -LiteralPath $archive -DestinationPath $toolsRoot -Force
	}
}
$nodeExe = Join-Path $NodeDirectory 'node.exe'
$actualNode = & $nodeExe --version
if ($LASTEXITCODE -ne 0 -or $actualNode -ne "v$nodeVersion") { throw "Expected Node v$nodeVersion; got $actualNode" }

if (-not $PnpmDirectory) {
	$PnpmDirectory = Join-Path $toolsRoot "pnpm-$pnpmVersion"
	if (-not (Test-Path -LiteralPath (Join-Path $PnpmDirectory 'bin/pnpm.mjs'))) {
		$metadata = Invoke-RestMethod -Uri "https://registry.npmjs.org/pnpm/$pnpmVersion"
		$archive = Join-Path $toolsRoot "pnpm-$pnpmVersion.tgz"
		Invoke-WebRequest -Uri $metadata.dist.tarball -OutFile $archive
		if (-not $metadata.dist.integrity.StartsWith('sha512-')) { throw 'Expected an npm SHA-512 integrity value.' }
		$hasher = [Security.Cryptography.SHA512]::Create()
		try { $integrity = 'sha512-' + [Convert]::ToBase64String($hasher.ComputeHash([IO.File]::ReadAllBytes($archive))) }
		finally { $hasher.Dispose() }
		if ($integrity -cne $metadata.dist.integrity) { throw 'pnpm archive integrity mismatch.' }
		New-Item -ItemType Directory -Path $PnpmDirectory -Force | Out-Null
		& tar -xzf $archive -C $PnpmDirectory --strip-components=1
		if ($LASTEXITCODE -ne 0) { throw 'Could not extract pnpm.' }
	}
}

# Use pnpm's maintained launcher directly. Older Corepack versions select a
# pnpm.cjs entry that does not exist in the pinned native pnpm distribution.
$pnpmLauncher = Join-Path $PnpmDirectory 'bin/pnpm.mjs'
$previousPath = $env:PATH
$previousPnpmHome = $env:PNPM_HOME
$previousFrozenLockfile = $env:PNPM_CONFIG_FROZEN_LOCKFILE
$previousRegistry = $env:npm_config_registry
$previousFoundrySkip = $env:FOUNDRY_LOCAL_SKIP_INSTALL
$previousBrowserSkip = $env:PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD
$previousNpmExecPath = $env:npm_execpath
$previousSkipPrelaunch = $env:VSCODE_SKIP_PRELAUNCH
try {
	$env:PATH = $NodeDirectory + ';' + $env:PATH
	$env:PNPM_HOME = Join-Path $toolsRoot 'pnpm-home'
	# Upstream postinstall installs additional roots. Propagate the policy to them.
	$env:PNPM_CONFIG_FROZEN_LOCKFILE = 'true'
	$env:npm_config_registry = 'https://registry.npmjs.org'
	$env:npm_execpath = $pnpmLauncher
	if ($Task -eq 'Install' -and $SkipDictationRuntime) {
		$env:FOUNDRY_LOCAL_SKIP_INSTALL = '1'
		Write-Warning 'Source preview: local dictation runtime is omitted and must be installed before validating dictation.'
	}
	if ($Task -eq 'Install' -and $SkipBrowserDownload) {
		$env:PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD = '1'
		Write-Warning 'Source preview: browser test runtime download is omitted; browser tests remain pending.'
	}
	$actualPnpm = & $nodeExe $pnpmLauncher --version
	if ($LASTEXITCODE -ne 0 -or $actualPnpm -ne $pnpmVersion) { throw "Expected pnpm $pnpmVersion; got $actualPnpm" }
	Write-Output "Node $actualNode; pnpm $actualPnpm; task $Task"
	Push-Location -LiteralPath $repoRoot
	try {
		switch ($Task) {
			'Install' { & $nodeExe $pnpmLauncher install --frozen-lockfile --registry=https://registry.npmjs.org --store-dir (Join-Path $toolsRoot 'store') }
			'Build' { & $nodeExe $pnpmLauncher run build-fast }
			'Package' {
				$sdkBinRoot = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits/10/bin'
				$sdkBin = Get-ChildItem -LiteralPath $sdkBinRoot -Directory -ErrorAction SilentlyContinue |
					Where-Object { $_.Name -match '^10\.\d+\.\d+\.\d+$' -and (Test-Path -LiteralPath (Join-Path $_.FullName 'x64/signtool.exe')) } |
					Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
				if (-not $sdkBin) { throw 'Windows SDK x64 signing tools are required for packaging.' }
				$env:PATH = (Join-Path $sdkBin.FullName 'x64') + ';' + $env:PATH
				$pendingChanges = & git status --porcelain
				if ($LASTEXITCODE -ne 0 -or $pendingChanges) { throw 'Commit the source changes before packaging so the embedded revision identifies the build.' }
				& $nodeExe $pnpmLauncher run gulp vscode-win32-x64
			}
			'Electron' { & $nodeExe $pnpmLauncher run electron }
			'Launch' {
				& $nodeExe build/lib/preLaunch.ts
				if ($LASTEXITCODE -ne 0) { throw 'Desktop prelaunch preparation failed.' }
				$env:VSCODE_SKIP_PRELAUNCH = '1'
				$launchArgs = @('--new-window', '--user-data-dir', (Join-Path $repoRoot '.build/aether-user-data'), '--extensions-dir', (Join-Path $repoRoot '.build/aether-extensions'))
				if ($OpenPath) { $launchArgs += $OpenPath }
				& ./scripts/code.bat @launchArgs
			}
		}
		if ($Task -ne 'Setup' -and $LASTEXITCODE -ne 0) { throw "$Task failed with exit code $LASTEXITCODE" }
	} finally { Pop-Location }
} finally {
	$env:PATH = $previousPath
	$env:PNPM_HOME = $previousPnpmHome
	$env:PNPM_CONFIG_FROZEN_LOCKFILE = $previousFrozenLockfile
	$env:npm_config_registry = $previousRegistry
	$env:FOUNDRY_LOCAL_SKIP_INSTALL = $previousFoundrySkip
	$env:PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD = $previousBrowserSkip
	$env:npm_execpath = $previousNpmExecPath
	$env:VSCODE_SKIP_PRELAUNCH = $previousSkipPrelaunch
}
