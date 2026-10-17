#Requires -Version 7.0
<#
.SYNOPSIS
  EveryDB を Release 構成でビルドして起動します。

.DESCRIPTION
  `npm run tauri build -- --no-bundle` でインストーラーを生成せずに
  Release 実行ファイルをビルドし、生成された everydb.exe を起動します。

.EXAMPLE
  pwsh -File .\dev\scripts\run-release-EveryDB-all.ps1
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

& (Join-Path $PSScriptRoot 'import-EveryDB-development-environment.ps1')

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$ExePath = Join-Path $RepoRoot 'src-tauri\target\release\everydb.exe'
Set-Location -LiteralPath $RepoRoot

Write-Host 'EveryDB release build and launch' -ForegroundColor Cyan
Write-Host "  root: $RepoRoot"
Write-Host '  cmd : npm run tauri build -- --no-bundle'
Write-Host ''

npm run tauri build -- --no-bundle
if ($LASTEXITCODE -ne 0) {
  throw "tauri release build failed with exit code $LASTEXITCODE"
}
if (-not (Test-Path -LiteralPath $ExePath -PathType Leaf)) {
  throw "Release build finished but executable was not found: $ExePath"
}

Write-Host "Starting: $ExePath" -ForegroundColor Green
Start-Process -FilePath $ExePath -WorkingDirectory (Split-Path -Parent $ExePath)
