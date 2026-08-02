#Requires -Version 7.0
<#
.SYNOPSIS
  EveryDB のリリース版を起動します。

.DESCRIPTION
  `src-tauri/target/release/everydb.exe` を起動します。
  実行ファイルが無い場合、または -Build 指定時は先に `npm run tauri build` を実行します。

.PARAMETER Build
  起動前に必ずリリースビルドを実行します。

.EXAMPLE
  pwsh -File .\dev\scripts\start-release.ps1

.EXAMPLE
  pwsh -File .\dev\scripts\start-release.ps1 -Build
#>
[CmdletBinding()]
param(
  [switch]$Build
)

$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Set-Location -LiteralPath $RepoRoot

$ExePath = Join-Path $RepoRoot 'src-tauri\target\release\everydb.exe'

function Invoke-ReleaseBuild {
  Write-Host "Building release..." -ForegroundColor Yellow
  Write-Host "  cmd: npm run tauri build"
  Write-Host ""
  npm run tauri build
  if ($LASTEXITCODE -ne 0) {
    throw "tauri build failed with exit code $LASTEXITCODE"
  }
  if (-not (Test-Path -LiteralPath $ExePath)) {
    throw "Release build finished but executable was not found: $ExePath"
  }
}

Write-Host "EveryDB release launch" -ForegroundColor Cyan
Write-Host "  root: $RepoRoot"
Write-Host "  exe : $ExePath"
Write-Host ""

if ($Build -or -not (Test-Path -LiteralPath $ExePath)) {
  if (-not $Build) {
    Write-Host "Executable not found. Building release first..." -ForegroundColor Yellow
  }
  Invoke-ReleaseBuild
}

Write-Host "Starting: $ExePath" -ForegroundColor Green
Start-Process -FilePath $ExePath -WorkingDirectory (Split-Path -Parent $ExePath)
