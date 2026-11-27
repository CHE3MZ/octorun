#!/usr/bin/env pwsh
# Install octorun from a GitHub release.
# Usage: pwsh install.ps1
#   OCTORUN_VERSION       release tag to install (defaults to the latest)
#   OCTORUN_INSTALL_DIR   directory to install into (defaults to ~/.local/bin)

$ErrorActionPreference = 'Stop'

$repo = 'CHE3MZ/octorun'
$version = $env:OCTORUN_VERSION
if (-not $version) {
  $latest = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest"
  $version = $latest.tag_name
}
if (-not $version.StartsWith('v')) {
  $version = "v$version"
}

$arch = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64' -or $env:PROCESSOR_ARCHITEW6432 -eq 'ARM64') { 'aarch64' } else { 'x86_64' }
$target = "$arch-pc-windows-msvc"
$asset = "octorun-$target.zip"
$url = "https://github.com/$repo/releases/download/$version/$asset"

$tmp = Join-Path $env:TEMP ([guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $tmp | Out-Null

try {
  Invoke-WebRequest -Uri $url -OutFile (Join-Path $tmp $asset)
  Invoke-WebRequest -Uri "$url.sha256" -OutFile (Join-Path $tmp "$asset.sha256")

  $expected = (Get-Content (Join-Path $tmp "$asset.sha256") -Raw).Split()[0].ToLower()
  $actual = (Get-FileHash (Join-Path $tmp $asset) -Algorithm SHA256).Hash.ToLower()
  if ($expected -ne $actual) {
    throw "checksum mismatch for $asset"
  }

  Expand-Archive -Path (Join-Path $tmp $asset) -DestinationPath $tmp -Force

  $installDir = if ($env:OCTORUN_INSTALL_DIR) { $env:OCTORUN_INSTALL_DIR } else { Join-Path $env:USERPROFILE '.local\bin' }
  New-Item -ItemType Directory -Force -Path $installDir | Out-Null
  Copy-Item (Join-Path $tmp 'octorun.exe') (Join-Path $installDir 'octorun.exe') -Force

  Write-Host "Installed octorun to $(Join-Path $installDir 'octorun.exe')"
  $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
  if (($userPath -split ';') -notcontains $installDir) {
    Write-Host "Add $installDir to your PATH to use octorun."
  }
} finally {
  Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}