#Requires -Version 7.0
<#
.SYNOPSIS
  EveryDB の運用スクリプト名を検査します。

.DESCRIPTION
  製品操作用スクリプトの動詞・モード・正式プロジェクト名・機能名と、
  モードを持たない補助スクリプトの許可名を検査します。
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$ProductPattern = '^(build|run|test|publish)-(debug|release|exe)-EveryDB-[A-Za-z0-9]+(?:-[A-Za-z0-9]+)?\.ps1$'
$AuxiliaryNames = @(
  'generate-EveryDB-access-sample.ps1',
  'import-EveryDB-development-environment.ps1',
  'test-development-environment.ps1',
  'test-script-naming.ps1'
)
$AllowedModes = @{
  build = @('debug', 'release')
  run = @('debug', 'release', 'exe')
  test = @('debug', 'release', 'exe')
  publish = @('release', 'exe')
}

$Errors = [System.Collections.Generic.List[string]]::new()
foreach ($File in Get-ChildItem -LiteralPath $PSScriptRoot -Filter '*.ps1' -File) {
  if ($File.Name -in $AuxiliaryNames) { continue }
  if ($File.Name -notmatch $ProductPattern) {
    $Errors.Add("命名規則に一致しません: $($File.Name)")
    continue
  }

  $Verb = $Matches[1]
  $Mode = $Matches[2]
  if ($Mode -notin $AllowedModes[$Verb]) {
    $Errors.Add("動詞 $Verb ではモード $Mode を使用できません: $($File.Name)")
  }
}

if ($Errors.Count -gt 0) {
  $Errors | ForEach-Object { Write-Error $_ }
  exit 1
}

Write-Output 'EveryDB の運用スクリプト命名検査に成功しました。'
