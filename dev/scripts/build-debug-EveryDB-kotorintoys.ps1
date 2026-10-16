#Requires -Version 7.0
<#
.SYNOPSIS
  KotorinToysから起動するEveryDB管理連携版をビルドします。
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Push-Location -LiteralPath $RepoRoot
try {
  if (-not (Test-Path -LiteralPath 'node_modules/.bin/tsc.cmd')) {
    npm ci
    if ($LASTEXITCODE -ne 0) { throw "npm ci failed with exit code $LASTEXITCODE" }
  }

  npm run build
  if ($LASTEXITCODE -ne 0) { throw "EveryDB frontend build failed with exit code $LASTEXITCODE" }

  cargo build --manifest-path src-tauri/Cargo.toml --target-dir src-tauri/target/kotorintoys --locked
  if ($LASTEXITCODE -ne 0) { throw "EveryDB KotorinToys integration build failed with exit code $LASTEXITCODE" }

  $Executable = Join-Path $RepoRoot 'src-tauri/target/kotorintoys/debug/everydb.exe'
  if (-not (Test-Path -LiteralPath $Executable -PathType Leaf)) {
    throw "EveryDB integration executable was not produced: $Executable"
  }
  Write-Host "EveryDB KotorinToys integration build succeeded: $Executable" -ForegroundColor Green
}
finally {
  Pop-Location
}
