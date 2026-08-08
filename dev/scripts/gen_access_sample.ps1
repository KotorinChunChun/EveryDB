# Access(accdb) のダミーデータ生成ツール（開発・検証用）。
# gen_samples.rs は accdb を作れない（Rust に accdb 書き込み手段が無い）ため、
# ACE OLEDB プロバイダ + ADOX を使う本スクリプトで samples/access_db.accdb を生成する。
#
# 実行: pwsh -File dev/scripts/gen_access_sample.ps1
# 前提: Microsoft.ACE.OLEDB.12.0 または 16.0 が登録済み（Office / Access Runtime）。

$ErrorActionPreference = "Stop"
$dbPath = Join-Path $PSScriptRoot "..\..\samples\access_db.accdb" | Resolve-Path -ErrorAction SilentlyContinue
if (-not $dbPath) { $dbPath = Join-Path $PSScriptRoot "..\..\samples\access_db.accdb" }
$dbPath = [System.IO.Path]::GetFullPath($dbPath)

if (Test-Path $dbPath) { Remove-Item $dbPath -Force }

# 利用可能な ACE OLEDB プロバイダを自動選択。
$provName = @("Microsoft.ACE.OLEDB.16.0", "Microsoft.ACE.OLEDB.12.0") |
    Where-Object { Test-Path "HKLM:\SOFTWARE\Classes\$_\CLSID" } |
    Select-Object -First 1
if (-not $provName) { throw "ACE OLEDB プロバイダが見つかりません（Access Runtime を導入してください）。" }
$provider = "Provider=$provName;Data Source=$dbPath;"

$cat = New-Object -ComObject ADOX.Catalog
[void]$cat.Create($provider)
$conn = New-Object -ComObject ADODB.Connection
$conn.Open($provider)
[void]$conn.Execute("CREATE TABLE 取引先 (ID INTEGER, 会社名 TEXT(100), 担当者 TEXT(50), 電話 TEXT(30), メモ MEMO)")
$rows = @(
    "1,'株式会社サンプル商事','田中太郎','03-1234-5678','主要取引先'",
    "2,'グローバル物流株式会社','鈴木花子','06-2345-6789','西日本担当'",
    "3,'テクノ開発合同会社','佐藤一郎','052-345-6789','システム開発'",
    "4,'みらいコンサルティング','高橋美咲','092-456-7890','九州エリア'",
    "5,'株式会社EveryDB','渡辺健','03-9876-5432','社内ツール検証用'"
)
foreach ($r in $rows) { [void]$conn.Execute("INSERT INTO 取引先 (ID,会社名,担当者,電話,メモ) VALUES ($r)") }
$conn.Close()
[System.Runtime.InteropServices.Marshal]::ReleaseComObject($conn) | Out-Null
[System.Runtime.InteropServices.Marshal]::ReleaseComObject($cat) | Out-Null
[GC]::Collect(); [GC]::WaitForPendingFinalizers()

Write-Output "生成成功: $dbPath ($((Get-Item $dbPath).Length) bytes / プロバイダ: $provName)"
