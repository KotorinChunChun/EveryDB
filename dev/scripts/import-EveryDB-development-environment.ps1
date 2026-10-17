#Requires -Version 7.0
<# 開発用環境変数を外部の秘密情報フォルダーから読み込む。値の評価や表示は行わない。 #>
[CmdletBinding()]
param(
    [string]$Path = 'C:\AI\env\EveryDB\development.env',
    [switch]$Required
)

$ErrorActionPreference = 'Stop'
if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
    if ($Required) { throw '開発用環境ファイルがありません。docs/development/secrets.md を確認してください。' }
    return
}

$entries = [Collections.Generic.Dictionary[string,string]]::new([StringComparer]::OrdinalIgnoreCase)
$lineNumber = 0
foreach ($line in [IO.File]::ReadAllLines($Path)) {
    $lineNumber++
    $trimmed = $line.Trim()
    if ($trimmed.Length -eq 0 -or $trimmed.StartsWith('#')) { continue }
    if ($trimmed -notmatch '^(EVERYDB_[A-Z0-9_]+)\s*=(.*)$') {
        throw "開発用環境ファイルの$lineNumber 行目は EVERYDB_変数名=値 の形式ではありません。"
    }
    $key = $Matches[1]
    $value = $Matches[2].Trim()
    if ($value.StartsWith('"') -or $value.StartsWith("'")) {
        if ($value.Length -lt 2 -or $value[-1] -ne $value[0]) { throw "開発用環境ファイルの$lineNumber 行目の引用符が閉じていません。" }
        $value = $value.Substring(1, $value.Length - 2)
    }
    if ($entries.ContainsKey($key)) { throw "開発用環境ファイルの$lineNumber 行目に重複する変数があります。" }
    $entries.Add($key, $value)
}

# 全行検証後に反映する。明示的に設定された既存のプロセス環境変数を優先する。
foreach ($entry in $entries.GetEnumerator()) {
    if ($entry.Value.Length -gt 0 -and [string]::IsNullOrEmpty([Environment]::GetEnvironmentVariable($entry.Key, 'Process'))) {
        [Environment]::SetEnvironmentVariable($entry.Key, $entry.Value, 'Process')
    }
}
