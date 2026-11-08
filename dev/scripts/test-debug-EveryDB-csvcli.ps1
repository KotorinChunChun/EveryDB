#Requires -Version 7.0
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$cli = Join-Path $root 'scripts/everydb-csv-cli.mjs'
$version = (Get-Content (Join-Path $root 'package.json') -Raw | ConvertFrom-Json).version
$run = Join-Path $root "dev/history/v$version/logs/csv-cli-$([Guid]::NewGuid().ToString('N'))"
$db = Join-Path $run 'isolated'
$normalFixture = Join-Path $run 'normal-fixture'
New-Item -ItemType Directory -Path $db, $normalFixture | Out-Null
Write-Output "検証出力: $run"

function Assert-Check($Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
    Write-Output "PASS: $Message"
}

function Invoke-CsvCli([string[]]$Arguments) {
    $json = & node $cli @Arguments
    if ($LASTEXITCODE) { throw "CLIが失敗しました: $($Arguments[0])" }
    return ($json -join "`n") | ConvertFrom-Json
}

$previousEncoding = [Console]::OutputEncoding
try {
    [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
    $first = Join-Path $run 'first.csv'
    $second = Join-Path $run 'second.csv'
    [IO.File]::WriteAllText($first, "品名,分類,備考`n青ペン,文具,`"一行目`n二行目`"`n紅茶,食品,対象外`n")
    [IO.File]::WriteAllText($second, "品名,分類,備考`nノート,文具,`nりんご,食品,対象外`n")
    $preview = Invoke-CsvCli @('preview', $first)
    Assert-Check ($preview.total_rows -eq 2 -and ($preview.columns -join ',') -ceq '品名,分類,備考' -and $preview.top_rows[0].values[0] -ceq '青ペン') 'CSVプレビューは列と先頭データを返す'
    Assert-Check (@(Get-ChildItem -LiteralPath $db -Force).Count -eq 0) 'プレビューは隔離DBを作成しない'

    $cache = Join-Path $normalFixture 'cache.db'
    [IO.File]::WriteAllText($cache, '通常DBを模した検証データ')
    $hash = (Get-FileHash -LiteralPath $cache).Hash
    & node $cli import $normalFixture $first '拒否対象' 2> (Join-Path $run 'normal-db-rejection.log') | Out-Null
    $rejected = $LASTEXITCODE -ne 0
    Assert-Check ($rejected -and (Get-FileHash -LiteralPath $cache).Hash -eq $hash -and -not (Test-Path (Join-Path $normalFixture 'csv-cli.sqlite3'))) 'cache.dbのある配置は拒否し原本を保持'

    $a = Invoke-CsvCli @('import', $db, $first, '文房具店')
    $b = Invoke-CsvCli @('import', $db, $second, '雑貨店')
    Assert-Check ($a.id -ne $b.id -and $a.record_count -eq 2 -and $b.record_count -eq 2) '独立した2ソースの取込を確定'
    $resultPath = Join-Path $run 'search.json'
    $result = & node $cli search $db '文具' $a.id $b.id
    if ($LASTEXITCODE) { throw '横断検索に失敗しました' }
    $result | Set-Content -LiteralPath $resultPath -Encoding utf8
    $hits = Get-Content -LiteralPath $resultPath -Raw | ConvertFrom-Json
    Assert-Check (!$hits.truncated -and $hits.rows.Count -eq 2 -and ($hits.rows.source_id -contains $a.id) -and ($hits.rows.source_id -contains $b.id)) '文具の2件が別プロセスから両ソースで見つかる'
    $single = Invoke-CsvCli @('search', $db, '文具', $b.id)
    Assert-Check ($single.rows.Count -eq 1 -and $single.rows[0].fields.品名 -ceq 'ノート') '指定外ソースの行は検索されない'

    $tsv = Join-Path $run 'result.tsv'
    $export = Invoke-CsvCli @('export', $resultPath, $tsv)
    $bytes = [IO.File]::ReadAllBytes($tsv)
    $lines = [Text.Encoding]::UTF8.GetString($bytes, 3, $bytes.Length - 3) -split "`r`n"
    Assert-Check (($bytes[0..2] -join ',') -eq '239,187,191' -and $export.rows -eq 2 -and $export.columns -eq 7 -and $lines.Count -eq 3 -and $lines[0] -ceq "#`tテーブル`t元行`t見出し`t品名`t分類`t備考") 'TSVはUTF-8 BOM・CRLF・見出し付き2行7列'
    Assert-Check ((@($lines | Where-Object { $_.EndsWith("`t青ペン`t文具`t一行目 二行目") }).Count -eq 1) -and (@($lines | Where-Object { $_.EndsWith("`tノート`t文具`t") }).Count -eq 1)) 'TSVの日本語・セル内改行・空欄を確認'
    $hash = (Get-FileHash -LiteralPath $tsv).Hash
    & node $cli export $resultPath $tsv 2> (Join-Path $run 'overwrite-rejection.log') | Out-Null
    $rejected = $LASTEXITCODE -ne 0
    Assert-Check ($rejected -and (Get-FileHash -LiteralPath $tsv).Hash -eq $hash) '同名TSVの上書きを拒否'
}
finally {
    [Console]::OutputEncoding = $previousEncoding
}
