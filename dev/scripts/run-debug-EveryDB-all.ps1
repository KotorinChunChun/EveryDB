#Requires -Version 7.0
<#
.SYNOPSIS
  EveryDB をデバッグモードで起動します（tauri dev）。

.DESCRIPTION
  リポジトリ直下で `npm run tauri dev` を実行します。
  Vite 開発サーバーと Rust デバッグビルドが立ち上がります。

.EXAMPLE
  pwsh -File .\dev\scripts\run-debug-EveryDB-all.ps1
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

& (Join-Path $PSScriptRoot 'import-EveryDB-development-environment.ps1')

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Set-Location -LiteralPath $RepoRoot

Write-Host "EveryDB debug launch" -ForegroundColor Cyan
Write-Host "  root: $RepoRoot"
Write-Host "  cmd : npm run tauri dev"
Write-Host ""

npm run tauri dev
if ($LASTEXITCODE -ne 0) {
  throw "tauri dev failed with exit code $LASTEXITCODE"
}
