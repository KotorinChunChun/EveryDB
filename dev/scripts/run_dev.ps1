# EveryDB 開発起動スクリプト（テスト用データ入りで起動）。
#
# 「cargo run」を src-tauri 直下で直に叩くと、Tauri の beforeDevCommand（Vite開発サーバー起動）
# を経由しないため WebView が http://localhost:1420 に接続できず
# ERR_CONNECTION_REFUSED になる。必ず「npm run tauri dev」（= Tauri CLI 経由）で
# Vite → cargo run の順に起動すること。本スクリプトはそれに加えて、起動前にテスト用の
# ローカルサンプルデータ（CSV/Excel/フォルダ/IME辞書/accdb）を生成し、実際にアプリが使う
# キャッシュ DB へ登録してから起動する。
#
# 実行: pwsh -File dev/scripts/run_dev.ps1
# Notion/SharePoint/EntraID/Teams も一緒に読み込みたい場合は、事前に
# $env:EVERYDB_NOTION_TOKEN 等を設定してから本スクリプトを実行する（README.md 参照）。

$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
Set-Location $root

Write-Output "===== 1/4: npm 依存関係 ====="
if (-not (Test-Path (Join-Path $root "node_modules"))) {
    npm install
} else {
    Write-Output "node_modules は既に存在するためスキップ"
}

Write-Output "===== 2/4: ローカルサンプルデータの生成 ====="
Push-Location (Join-Path $root "src-tauri")
try {
    cargo run --example gen_samples
} finally {
    Pop-Location
}

# accdb サンプルは ACE OLEDB(Office/Access Runtime) が無い環境では作れないため、
# 失敗してもスクリプト全体は止めない(ベストエフォート)。
try {
    pwsh -File (Join-Path $root "dev\scripts\gen_access_sample.ps1")
} catch {
    Write-Warning "accdb サンプル生成をスキップ(ACE OLEDB 未導入の可能性): $($_.Exception.Message)"
}

Write-Output "===== 3/4: テスト用データソースをキャッシュ DB へ登録 ====="
# 先に一度アプリを起動して cache.db を作っておく必要があるため、無ければここで初期化を兼ねて
# 空実行し、以降はローカルサンプル(+ 環境変数があればクラウド系も)を登録する。
Push-Location (Join-Path $root "src-tauri")
try {
    cargo run --example seed_test_sources
} finally {
    Pop-Location
}

Write-Output "===== 4/4: アプリ起動(npm run tauri dev) ====="
npm run tauri dev
