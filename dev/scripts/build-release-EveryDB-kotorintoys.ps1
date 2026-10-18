#Requires -Version 7.0
<#
.SYNOPSIS
  共通インストーラー用のネイティブReleaseだけをビルドします。
#>
[CmdletBinding()]
param([string]$TargetDirectory)
$ErrorActionPreference = 'Stop'
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
if (-not $TargetDirectory) { $TargetDirectory = Join-Path $RepoRoot 'src-tauri/target' }
$TargetDirectory = [IO.Path]::GetFullPath($TargetDirectory)
$PreviousTarget = $env:CARGO_TARGET_DIR
$PreviousFlags = $env:CARGO_ENCODED_RUSTFLAGS
Push-Location -LiteralPath $RepoRoot
try {
  if (-not (Test-Path -LiteralPath 'node_modules/.bin/tauri.cmd')) {
    npm ci
    if ($LASTEXITCODE) { throw 'npm ci に失敗しました。' }
  }
  $env:CARGO_TARGET_DIR = $TargetDirectory
  $separator = [char]31
  $flags = @()
  if ($PreviousFlags) { $flags += $PreviousFlags.Split($separator) }
  elseif ($env:RUSTFLAGS) { throw '再現可能な配布ではRUSTFLAGSではなくCARGO_ENCODED_RUSTFLAGSを使用してください。' }
  $flags += "--remap-path-prefix=$env:USERPROFILE=USERPROFILE", "--remap-path-prefix=$RepoRoot=SOURCE"
  $env:CARGO_ENCODED_RUSTFLAGS = $flags -join $separator
  npm run tauri -- build --no-bundle -- --locked
  if ($LASTEXITCODE) { throw 'EveryDBのReleaseビルドに失敗しました。' }
  $Executable = Join-Path $TargetDirectory 'release/everydb.exe'
  if (-not (Test-Path -LiteralPath $Executable -PathType Leaf)) { throw 'Release実行ファイルがありません。' }
  Write-Output "共通配布用ビルド完了: $Executable"
}
finally {
  $env:CARGO_TARGET_DIR = $PreviousTarget
  $env:CARGO_ENCODED_RUSTFLAGS = $PreviousFlags
  Pop-Location
}
