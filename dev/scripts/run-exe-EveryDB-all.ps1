#Requires -Version 7.0
<#
.SYNOPSIS
  既存の EveryDB Release 実行ファイルを起動します。

.DESCRIPTION
  `src-tauri/target/release/everydb.exe` をビルドせず起動します。
  実行ファイルが無い場合は失敗し、暗黙にビルドしません。

.EXAMPLE
  pwsh -File .\dev\scripts\run-exe-EveryDB-all.ps1
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

& (Join-Path $PSScriptRoot 'import-EveryDB-development-environment.ps1')

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Set-Location -LiteralPath $RepoRoot

$ExePath = Join-Path $RepoRoot 'src-tauri\target\release\everydb.exe'

Write-Host "EveryDB existing executable launch" -ForegroundColor Cyan
Write-Host "  root: $RepoRoot"
Write-Host "  exe : $ExePath"
Write-Host ""

if (-not (Test-Path -LiteralPath $ExePath -PathType Leaf)) {
  throw "Release executable was not found. Build it explicitly before using exe mode: $ExePath"
}

Write-Host "Starting: $ExePath" -ForegroundColor Green
Start-Process -FilePath $ExePath -WorkingDirectory (Split-Path -Parent $ExePath)
