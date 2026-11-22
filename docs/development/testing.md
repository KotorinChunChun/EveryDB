# テスト方法

## セキュリティ回帰（ソース0.1.2）

公開CSVの受信／ローカル入力／ZIP内CSVは256MiB、ZIPの項目数は4096、公開CSVは100万行を上限とし、宣言サイズだけでなく実際の読取り量も確認する。郵便番号のような10万行超の正規データを拒否しない回帰を含む。Excelの自前XML読取りにも容量上限を適用する。HTTP失敗は応答本文・URLの生値をエラーへ転送しない。

`npm run check:all`で境界・秘密値マスク・架空接続先・型・文書・CSP・fmt・clippy・ビルドを確認する。Rust 1.88以上を使用する。npm/Cargoの公開アドバイザリは別途最新DBで照合し、保守終了やWindowsで未使用のGTK依存を0件へ隠さず報告する。実サービス資格情報・通常プロファイルはテスト入力にしない。

## 通常の自動検証

```powershell
npm run check:all
```

`check:all` は TypeScript型検査、Vitest、公開文書検査、本番フロントビルド、Rustのformat、Clippy、ライブラリテスト、バイナリビルドを順に実行します。外部サービスの認証情報やWindows Helloは使用しません。

Rustテストの前に、追跡された生成コードと同梱CSVから架空のサンプル一式を `dev/history/v<版>/logs/offline-quality-<識別子>/samples/` へ生成します。テストは `EVERYDB_TEST_SAMPLES_DIR` によりその出力だけを参照します。開発者の既存 `samples/` と秘密情報は読み込みません。GitHub CIも同じコマンドを使います。

開発用環境ファイルの安全な読込みは `pwsh -File dev/scripts/test-development-environment.ps1` で確認します。ライブ試験の秘密情報は [開発用秘密情報](secrets.md) に従い `C:\AI\env\EveryDB\` へ保存してください。

運用スクリプトの命名だけを確認する場合は次を実行します。

```powershell
pwsh -File .\dev\scripts\test-script-naming.ps1
```

## CSVの取込からTSV出力までのCLI検証

Node.js 22.18以降と、PATHに登録済みのRust/Cargoが必要です。EveryDBのリポジトリ直下で、次の一括検証を実行できます。

```powershell
pwsh -File .\dev\scripts\test-debug-EveryDB-csvcli.ps1
```

架空のCSVを2つ生成し、プレビュー、取込確定、両ソースを対象にした検索、TSV出力を別々のCLI呼び出しで確認します。成功時は終了コード0です。生成されたCSV、DB、検索JSON、TSVと拒否試験のログは、表示された `dev/history/v<版>/logs/csv-cli-<識別子>/` 内にあります。

個別に再現するには、次のように実行します。CSVは既定のヘッダーあり、カンマ区切り、文字コード自動判定を使用します。

```powershell
$version = (Get-Content package.json -Raw | ConvertFrom-Json).version
$run = Join-Path (Get-Location) "dev/history/v$version/logs/csv-cli-$([Guid]::NewGuid().ToString('N'))"
$db = Join-Path $run 'isolated'
New-Item -ItemType Directory -Path $db -Force | Out-Null
$first = Join-Path $run 'first.csv'
$second = Join-Path $run 'second.csv'
Set-Content -LiteralPath $first -Encoding utf8 -Value '品名,分類','青ペン,文具'
Set-Content -LiteralPath $second -Encoding utf8 -Value '品名,分類','ノート,文具'

$PSNativeCommandUseErrorActionPreference = $true
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
$cli = './scripts/everydb-csv-cli.mjs'
node $cli preview $first
$a = node $cli import $db $first '文房具店' | ConvertFrom-Json
$b = node $cli import $db $second '雑貨店' | ConvertFrom-Json
$result = Join-Path $run 'search.json'
node $cli search $db '文具' $a.id $b.id | Set-Content -LiteralPath $result -Encoding utf8
node $cli export $result (Join-Path $run 'result.tsv')
```

- プレビューはDBを開きません。取込・検索は明示した既存の絶対パスにある `csv-cli.sqlite3` だけを使い、通常の `cache.db` がある配置は拒否します。試験ごとに空の専用フォルダーを使ってください。アプリ起動や検索履歴の保存は行いません。
- 取込はGUIと同じ登録・初回読込み経路です。初回読込みが失敗した場合は終了コードが非0になり、GUIと同様に失敗したソースが隔離DBに残る場合があります。
- 検索は取込で返ったソースIDを明示し、結果をJSONで返します。TSVはそのJSONを読み、GUIと同じ既定列・セル変換・TSV変換を使ってUTF-8 BOM付きで新規保存します。空・件数上限に達した結果と既存ファイルへの上書きは拒否します。GUIで変更した列順やソートは読み込みません。

## ライブテスト

Rustの一部のテストは外部サービス、ローカルブラウザプロファイル、ODBCドライバ、認証情報を必要とするため `ignored` です。秘密情報はリポジトリや設定ファイルへ保存せず、実行時の環境変数だけで渡してください。

## 手動確認

Tauri実機確認、Windows Hello、インストーラーでの更新・アンインストールは対話操作を伴います。リリース候補の最終段階で実施し、確認できなかった項目は未検証として報告書へ残します。

## DB 変更前バックアップと復旧の確認

`cache.db` のスキーマ更新は、起動時に `open_cached_database` が WAL を checkpoint したうえで
`cache.db.before-migration.bak` を作成し、移行成功後に削除します。失敗時はバックアップから
元ファイルへ復旧してからエラーを返すため、実データを破壊したまま起動しない設計です。

### 自動テスト（秘密値不要）

```powershell
cargo test --manifest-path src-tauri\Cargo.toml --lib migrates_legacy migrates_version1 file_migration
```

- `legacy_cache_v1.sql` … sort_order / row_number 無しの v0.0.1 相当
- `legacy_cache_v1_step1.sql` … schema_version=1（sort_order あり、row_number 無し）
- ファイル DB では成功時の backup 削除と、強制失敗時の復旧を検証

### 手動確認（本番 DB を触る場合）

1. EveryDB を終了する。
2. `%APPDATA%\com.everydb.app\cache.db` を別名コピーして退避する（例: `cache.db.manual.bak`）。
3. 開発ビルドまたはインストーラ更新後の build でアプリを起動する。
4. 接続一覧・検索結果・保存済みトークン参照が維持されていることを確認する。
5. 問題があれば EveryDB を終了し、退避コピーで `cache.db` を上書き復元する。
6. 起動失敗で `cache.db.before-migration.bak` が残っている場合は、内容を確認したうえで
   手動で `cache.db` へ戻してから再起動する（上書き前に必ず別名保存する）。

fixture は `src-tauri/tests/fixtures/` にあり、個人情報・認証情報は含みません。

## 依存関係の脆弱性監査（RF-60-11）

```powershell
npm audit --omit=dev --audit-level=high
cargo audit --file src-tauri\Cargo.lock
```

- **npm（本番依存）**: 2026-10-17 の監査で検出0件。
- **Rust**: `cargo-audit` が必要（未導入の環境では `cargo install cargo-audit --locked`）。2026-10-17の公開前監査ではrustlsを0.23.46へ更新し、TLS関連4件を解消した。quick-xmlは0.41.0へ更新済み。
  - ZIP 3アドバイザリ（RUSTSEC-2026-0367/0368/0369）が、ロック内3版に対し計9件残る。指摘対象はファイルシステムへの展開API。EveryDBはZIPエントリーをメモリーへ読むだけで、calamineも展開APIを呼ばないことをソースで確認した。2.4系はテスト用rust_xlsxwriter経由で、製品依存ではない。監査結果を0件とは扱わず、リリースノートと版別報告へ残す。
  - unmaintained／unsoundとyankedの警告も残る。詳細な判定と生ログの所在は [v0.1.0実装報告](../../dev/history/v0.1.0/v0.1.0-imp.md) を参照する。
- `check:all` には **cargo audit を含めない**（ネットワークと advisory DB 更新が必要なため）。リリース前の手動確認項目とする。

## ignored live テスト一覧（RF-80-07/08）

`cargo test --lib` では **16 件 ignored**（2026-08-02）。実行例:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml --lib -- --ignored notion_live
```

| テスト（フィルタキーワード） | 必要なもの |
| :--- | :--- |
| `notion_live` | `EVERYDB_NOTION_TOKEN`（+ DB ID 等） |
| `airtable_live` | `EVERYDB_AIRTABLE_TOKEN` / `_BASE` / `_TABLE` |
| `graph_live` | `EVERYDB_GRAPH_*` + admin-consented app |
| `gsheets_live` | `EVERYDB_GOOGLE_KEY_PATH` + `EVERYDB_GSHEET_ID` |
| `ical_live` | `EVERYDB_ICAL_URL` |
| `browser_live` / `browser_profiles_live` | 実 Chrome/Edge/Firefox プロファイル |
| `accdb` / `list_accdb` | Microsoft Access ODBC + `samples/access_db.accdb` |
| `postal_code_live` / `corporate_number_live` | ネットワーク（政府公開データ DL） |
| `zenn_search_api_live` | ネットワーク（zenn.dev API） |
| `browse` graph/notion live | 上記 Graph/Notion 資格情報 |

認証情報はリポジトリへ保存せず、実行時の環境変数のみで渡す。

## 性能ベースライン（RF-50-08）

オフラインの FTS 検索ベースラインは Rust 単体テストで計測します（外部認証・実 cache.db 不要）。

```powershell
cargo test --manifest-path src-tauri\Cargo.toml --lib perf_baselines -- --nocapture
```

- `baseline_search_10k_rows` … 10,000 行 seed 後の FTS 検索（閾値 3 秒）
- `baseline_search_100k_rows` … 100,000 行 seed 後の FTS 検索（閾値 15 秒）
- `baseline_reload_100_sources` … 100 ソース一括 reload（閾値 120 秒、末尾 FTS optimize 1 回）

計測値はテスト stderr に `RF-50-08 …` 形式で出力されます。2026-08-02 開発環境（debug / in-memory）の参考値: search_10k ≈ 0.45ms、search_100k ≈ 0.46ms、reload_100_sources ≈ 71ms（100/100 成功）。

フロント grid 10k 行は Vitest `features/grid/data.test.ts` の `RF-50-08: column build and sort stay fast for 10k rows`（閾値 2s、列構築＋ソートのみ）。DOM 仮想スクロール実描画は RF-80-15。
