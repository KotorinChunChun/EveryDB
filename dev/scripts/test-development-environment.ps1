#Requires -Version 7.0
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$logs = Join-Path $repo 'dev/history/v0.1.0/logs'
$testRoot = Join-Path $logs ('environment-parser-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $testRoot -Force | Out-Null
$path = Join-Path $testRoot 'synthetic.env'
$loader = Join-Path $PSScriptRoot 'import-EveryDB-development-environment.ps1'
$names = @('EVERYDB_QA_LITERAL', 'EVERYDB_QA_EXISTING', 'EVERYDB_QA_ATOMIC')
$original = @{}
foreach ($name in $names) { $original[$name] = [Environment]::GetEnvironmentVariable($name, 'Process'); [Environment]::SetEnvironmentVariable($name, $null, 'Process') }
$checks = 0
function Assert-Check([bool]$Condition, [string]$Label) {
    if (-not $Condition) { throw "環境読込み検査失敗: $Label" }
    $script:checks++
}
try {
    $env:EVERYDB_QA_EXISTING = 'existing-fixture'
    $literal = '$(throw "値を実行してはいけない")'
    @('# 架空データのみ', "EVERYDB_QA_LITERAL='$literal'", 'EVERYDB_QA_EXISTING=replacement-fixture') | Set-Content -LiteralPath $path
    & $loader -Path $path -Required
    Assert-Check ($env:EVERYDB_QA_LITERAL -eq $literal) '値を評価せず読込む'
    Assert-Check ($env:EVERYDB_QA_EXISTING -eq 'existing-fixture') '既存環境変数を優先'

    @('EVERYDB_QA_ATOMIC=fixture', 'PATH=forbidden-fixture') | Set-Content -LiteralPath $path
    $rejected = $false
    try { & $loader -Path $path } catch { $rejected = $true }
    Assert-Check $rejected 'EVERYDB以外の変数を拒否'
    Assert-Check ([string]::IsNullOrEmpty($env:EVERYDB_QA_ATOMIC)) '形式不正なら部分反映しない'

    @('EVERYDB_QA_ATOMIC=first-fixture', 'EVERYDB_QA_ATOMIC=second-fixture') | Set-Content -LiteralPath $path
    $rejected = $false
    try { & $loader -Path $path } catch { $rejected = $true }
    Assert-Check $rejected '重複変数を拒否'
    'EVERYDB_QA_ATOMIC="unclosed-fixture' | Set-Content -LiteralPath $path
    $rejected = $false
    try { & $loader -Path $path } catch { $rejected = $true }
    Assert-Check $rejected '閉じていない引用符を拒否'
    $rejected = $false
    try { & $loader -Path (Join-Path $testRoot 'absent.env') -Required } catch { $rejected = $true }
    Assert-Check $rejected '必須ファイルの欠落を拒否'
    Write-Output "開発用環境ファイル検査: PASS $checks"
} finally {
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name, $original[$name], 'Process') }
}
