# EveryDB

各種データソースをローカル SQLite にキャッシュし、横断的に高速検索・特定・入力するツール。
要件は [docs/requirements.md](docs/requirements.md) を参照。

## 現在の実装状況: 全データソース + 運用性強化 + UX 改善 実装完了（M1〜M12 + 第3弾）

### 第3弾: UX 改善・バグ修正
- **結果表**: 横スクロール時に列見出しが本体に追従(`scrollLeft` 同期)。行数上限を
  ツールバーから設定可能(既定 1000)。**TSV エクスポート**(`tauri-plugin-dialog` の保存ダイアログ
  + `tauri-plugin-fs`)。
- **ファイルパス**: 両端にダブルクォートが付いていても正規化して読める(フロント/バックエンド
  両方で対応、`connectors::strip_path_quotes`)。全 path 系フィールドに**参照ボタン**
  (ファイル/フォルダ選択ダイアログ)を追加。
- **Excel: セル範囲**(旧「単一セル」を全面刷新): `A1:C10` 形式の範囲をテーブルとして読み込み
  (先頭行=ヘッダー)。単一セル指定にも後方互換。
- **データソース追加のテーブル一括選択**: チェック検知範囲を行全体に拡大、全選択/全解除
  ボタン追加。一括追加が失敗した場合はダイアログを閉じずエラーを表示(サイレント失敗の防止)。
- **サイドバーのツリー化**: 接続先(パス・サイト URL・Notion・Microsoft 365)をツリーの親、
  個々のテーブルを子として表示。接続先チェックボックスで配下テーブルを一括 ON/OFF(tri-state)。
  テーブル名でのインクリメンタルサーチ、ドラッグ並び替えも継続対応。
  （用語の定義は [docs/requirements.md](docs/requirements.md#0-用語) を参照）
- **右クリックメニュー**: 欄外クリック・ESC・コマンド選択で確実に閉じるよう修正
  (`.ctx-menu` の CSS `display:flex` が `[hidden]` を上書きしていた表示バグを解消)。
  ラベルを「編集」「再読込」に変更。

## 実装状況: 全データソース + 運用性強化 実装完了（M1〜M12）

### 運用性・検索 UX 強化（M7〜M12・第2弾要求）
- **マスターパスワード**（初回起動時に必須設定。シークレットを「表示」する操作の直前に要求し、
  一度入力すれば 10 分間は再要求しない。argon2 でハッシュ化しローカル保存。DPAPI 暗号化とは
  独立した UI 表示ゲート）
- **ソース管理 UI 強化**: 右クリックメニュー（表示/編集・複製・リロード・削除）、
  ドラッグ&ドロップで並び替え、全選択/全解除、**Alt+クリックでソロ選択**、ソース単位の
  即時リロードボタン
- **ブラウズして追加**: Notion データベース一覧から選択、SharePoint サイト/Teams チーム一覧
  → 中のリスト/ライブラリを選ぶだけで追加、Excel/SQLite/Access はテーブル・シート一覧から
  複数チェックで一括追加
- **表示・検索カスタマイズ**: ソースごとに列の表示 ON/OFF、検索対象列の選択、
  列に表示名（エイリアス）を付けて複数ソースの同名列を自動統合表示
- **Everything ライクな常駐 UX**: 既定でタスクトレイ常駐、Windows 自動起動（設定で切替）、
  **Alt+Ctrl+D** のグローバルホットキーでウィンドウ表示、検索クエリは半角スペース区切りで
  AND 検索、検索履歴の保存・再利用
- ウィンドウタイトルを「いつでも、どこでも、データをちゅんちゅん - EveryDB」に変更

### 第4弾: UX 強化 + データソース拡充（全 29 種）
- **ズーム**（Ctrl+ / Ctrl- / Ctrl0、設定で倍率保持）、**テーマ切替**（ダーク/ライト/ナイト）
- **マルチフォーマットコピー**（右クリック: 値 / 行TSV / 見出し付きTSV(空列除外) / JSON。Ctrl+C は値コピー維持）
- **検索プリセット・お気に入り**（クエリ+選択ソースを保存・ワンクリック復元）
- **追加データソース**（すべてライブ検証済み。Box/PostgreSQL/MySQL は実装のみ）:
  - ブラウザ **閲覧履歴 / ブックマーク**（Chrome/Edge/Brave/Firefox、ロック回避コピー）
  - **EntraID デバイス / Intune デバイス / Microsoft 365 ライセンス**（Graph）
  - **Outlook 予定表 / OneDrive / Teams タスク&Planner**（Graph、対象ユーザー/グループ指定）
  - **Google スプレッドシート**（サービスアカウント JWT 認証）
  - **Google カレンダー / iCal**（公開 .ics URL、認証不要）
  - **Airtable**（Personal Access Token）
  - **PostgreSQL / MySQL**（ホスト/DB/テーブル指定）
  - **Box**（Client Credentials Grant、ファイル/フォルダ再帰一覧）

### 基盤（M1）
- Tauri v2 + Rust + TypeScript の土台
- SQLite キャッシュ（`sources` / `records` / `records_fts`。FTS5 trigram + external content）
- コネクタ抽象化（`Connector` trait）
- インクリメンタルサーチ（debounce + シーケンス番号で最新のみ採用、仮想スクロール）
  - 空クエリ = ブラウズ / 1〜2 文字 = LIKE / 3 文字以上 = FTS trigram + bm25 + ソース別上限
- 単一セル選択 + Ctrl+C コピー、**矢印キーでセル移動 / Enter で開く**（Excel 風操作）
- 選択レコードの「開く」（ローカルパス = 関連付けアプリ / URL = 既定ブラウザ）
- データソースの追加・即時リロード（スナップショット全置換 + ステータス表示）
- **ソース管理**（削除 = キャッシュもカスケード削除 / ソース別の更新間隔[分]を編集）

### 対応データソース（M1 + M2）
- **CSV**（ヘッダ有無）
- **ローカル SQLite**（テーブル指定）
- **Excel: シートのテーブル**（先頭行ヘッダ）
- **Excel: 単一セル**（A1 形式指定）
- **Excel: シート名一覧**
- **フォルダのファイル一覧**（再帰 / 拡張子フィルタ、名前・パス・拡張子・サイズ・更新日時）
- **IME辞書: Microsoft IME**（エクスポート済み UTF-16LE TSV）
- **IME辞書: Google 日本語入力**（エクスポート済み UTF-8 TSV）
- **Access (accdb / mdb)**（odbc-api + ACE ODBC ドライバ、テーブル指定、UTF-16 で Unicode 安全に取得）
- **Notion データベース**（ureq + インテグレーショントークン、プロパティ型を網羅変換、ページURL=open_ref、ページング対応）
- **EntraID ユーザー / グループ**（Microsoft Graph、クライアントクレデンシャル認証）
- **Teams チーム一覧**（Graph、Team 化グループ）
- **SharePoint リスト**（Graph、システム列を除外して表示、item webUrl=open_ref）
- **SharePoint ドキュメントライブラリ**（Graph、フォルダ再帰、driveItem webUrl=open_ref）

### 常駐・自動更新（M3）
- **トレイ常駐**（表示 / 全ソース更新 / 終了メニュー）。ウィンドウの×はトレイに格納。
- **内部タイマー**（60 秒 tick）で `refresh_interval` 経過ソースを自動スナップショット更新。
- 更新完了で `sources-updated` を emit → フロントがサイドバー（件数・鮮度）を更新。

**シークレット保護**: Notion トークン / Graph クライアントシークレットは Windows DPAPI で暗号化して
保存（`config_json` に `dpapi:<hex>` 形式、平文では持たない）。`src-tauri/src/secret.rs`。

### 外部前提
- **accdb**: Microsoft Access ODBC ドライバ（`Microsoft Access Driver (*.mdb, *.accdb)`）が必要
  （Office / Access Runtime に同梱）。アプリのビルド（64bit）とドライバのビット数を揃えること。
- **Graph 系**: Azure AD アプリ登録 + アプリケーション許可（`Sites.Read.All` / `User.Read.All` /
  `Group.Read.All`）+ 管理者同意。テナントID / クライアントID / クライアントシークレット(値)を入力。
- **Notion**: インテグレーショントークン + 対象 DB をインテグレーションに共有。

## ダミーデータ生成ツール（開発用）

検証用のサンプル（xlsx / IME 辞書 / フォルダツリー）を生成:

```bash
cd src-tauri && cargo run --example gen_samples
```

`samples/` に `employees.xlsx` / `msime_dict.txt`（UTF-16LE）/ `google_dict.txt`（UTF-8）/
`tree/`（ネストしたファイル群）を生成する。

Access のサンプル（`samples/access_db.accdb`）は Rust では作れないため、ACE OLEDB を使う
PowerShell スクリプトで生成する:

```bash
pwsh -File scripts/gen_access_sample.ps1
```

## 開発用データソースの一括登録（毎回の手入力を省く）

Notion / SharePoint / EntraID / Teams は認証情報が長く、UI から毎回手入力するのは煩雑なため、
実際にアプリが使うキャッシュ DB（`%APPDATA%\com.everydb.app\cache.db`）へ直接登録するツールを用意した。
`add_source` と全く同じ経路（DPAPI 暗号化 → 初回 fetch）を通るため、動作は通常の追加と同一。
秘密情報はスクリプトに書かず、実行時の環境変数からのみ渡す（このリポジトリには一切残らない）。

```bash
cd src-tauri
$env:EVERYDB_NOTION_TOKEN='ntn_...'
$env:EVERYDB_NOTION_DB='...'
$env:EVERYDB_GRAPH_TENANT='...'
$env:EVERYDB_GRAPH_CLIENT='...'
$env:EVERYDB_GRAPH_SECRET='...'
$env:EVERYDB_SP_SITE='https://.../sites/xxx'
cargo run --example seed_test_sources
```

指定が無いソース種別はスキップされる。既に同名・同種のソースが存在する場合は削除してから
再登録するため、値を変えて再実行しても重複しない。次回アプリを起動すればソース選択に
そのまま表示される。

## 開発・起動

**推奨: テスト用データ入りで一発起動**

```bash
pwsh -File scripts/run_dev.ps1
```

npm 依存関係の確認 → ローカルサンプル生成([gen_samples.rs](src-tauri/examples/gen_samples.rs) /
[gen_access_sample.ps1](scripts/gen_access_sample.ps1)) → 実キャッシュ DB への一括登録
([seed_test_sources.rs](src-tauri/examples/seed_test_sources.rs)、CSV/Excel/フォルダ/IME辞書/
accdb を無条件で登録。Notion/Graph 系は該当の環境変数が設定されていれば追加登録) →
`npm run tauri dev` の順に実行する。起動後はソースが最初から入った状態で検索できる。

`sample2/`（リポジトリ直下、gitignore 対象・各自で配置）に置いたファイルも同じスクリプトが
自動検出して登録する: `0804koukinsisyutsu.xlsx` / `0805koukinsisyutsu.xlsx`
（シート「公金支出情報一覧表」を excel_table として）、`Chinook_Sqlite.sqlite`
（11 テーブルすべてを個別の sqlite ソースとして）。

**手動での起動**

```bash
npm install
npm run tauri dev      # 開発起動（ウィンドウが開く）
npm run tauri build    # リリースビルド
```

> ⚠️ `src-tauri` 直下で `cargo run` を直接実行すると、Tauri の `beforeDevCommand`
> （Vite 開発サーバー起動）を経由しないため WebView が `http://localhost:1420` に接続できず
> **ERR_CONNECTION_REFUSED** になる。必ず `npm run tauri dev`（または上記スクリプト）を使うこと。

バックエンドのテスト:

```bash
cd src-tauri && cargo test
```

## 使い方

1. 初回起動時、**マスターパスワードの設定を求められる**（キャンセル不可）。これを設定しないと
   Notion/SharePoint 等シークレットを扱うソースは追加できない。
2. 右上「＋ ソース追加」→ 種別「CSV ファイル」、パスに
   `C:\develop\EveryDB\samples\customers.csv` を指定して追加。
3. 検索ボックスに入力すると即時に絞り込み表示（半角スペース区切りで複数語 AND 検索）。
4. セルをクリックして選択、Ctrl+C で単一セルコピー、矢印キーで移動、Enter で開く。
5. 「開く」で選択レコードのファイルを開く。「即時リロード」で再取得、ソース単位のリロードは
   サイドバー各行の ⟳ ボタンから。
6. サイドバーのソースを右クリックすると、表示/編集・複製・削除ができる。
7. **Alt+Ctrl+D** でどこからでもウィンドウを呼び出せる（既定でタスクトレイ常駐）。

キャッシュ DB は OS のアプリデータディレクトリ（`%APPDATA%\com.everydb.app`）に作成される。

## 既知の制約

フロントエンドは Vite 開発サーバー上で DOM 構造・動的フィールド生成・イベント配線を検証済みだが、
実際の Tauri ネイティブウィンドウでの通し操作（マスターパスワード設定 → ソース追加 → 検索 →
ブラウズピッカーの実データ表示等）は未実施。`npm run tauri dev` で実機確認を推奨する。
