# テスト方法

## 通常の自動検証

```powershell
npm run check:all
```

`check:all` は TypeScript型検査、Vitest、公開文書検査、本番フロントビルド、Rustのformat、Clippy、ライブラリテスト、バイナリビルドを順に実行します。外部サービスの認証情報やWindows Helloは使用しません。

Rustテストの前に、追跡された生成コードと同梱CSVから架空のサンプル一式を `dev/history/v<版>/logs/offline-quality-<識別子>/samples/` へ生成します。テストは `EVERYDB_TEST_SAMPLES_DIR` によりその出力だけを参照します。開発者の既存 `samples/` と秘密情報は読み込みません。GitHub CIも同じ入口を使います。

開発用環境ファイルの安全な読込みは `pwsh -File dev/scripts/test-development-environment.ps1` で確認します。ライブ試験の秘密情報は [開発用秘密情報](secrets.md) に従い `C:\AI\env\EveryDB\` へ保存してください。

運用スクリプトの命名だけを確認する場合は次を実行します。

```powershell
pwsh -File .\dev\scripts\test-script-naming.ps1
```

## ライブテスト

Rustの一部テストは外部サービス、ローカルブラウザプロファイル、ODBCドライバ、認証情報を必要とするため `ignored` です。秘密情報はリポジトリや設定ファイルへ保存せず、実行時の環境変数だけで渡してください。

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
cargo audit --manifest-path src-tauri\Cargo.toml
```

- **npm（本番依存）**: 2026-08-02 時点で high 以上 0 件。
- **Rust**: `cargo-audit` が必要（未導入の環境では `cargo install cargo-audit --locked`）。2026-08-02 実行結果:
  - **要判断**: `quick-xml` 0.31.0 — RUSTSEC-2026-0194/0195（high、>=0.41.0 へ更新）。Tauri/GTK 経由の transitive 警告（unmaintained/unsound）は多数。major 更新は別タスクで判断。
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
