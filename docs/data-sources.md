# データソース一覧と接続手順

この一覧は `shared/source-types.json` を正本としており、文書検査で画面に表示する全種別と同期を確認します。

<!-- source-catalog:start -->
| 区分 | データソース | 接続手順 |
| --- | --- | --- |
| ローカル | CSV ファイル（`csv`） | [local-files](guides/local-files.md) |
| ローカル | SQLite データベース（`sqlite`） | [local-files](guides/local-files.md) |
| ローカル | Access (accdb / mdb)（`accdb`） | [local-files](guides/local-files.md) |
| ローカル | Excel: テーブル(ListObject)（`excel_table`） | [local-files](guides/local-files.md) |
| ローカル | Excel: シート(セル範囲)（`excel_range`） | [local-files](guides/local-files.md) |
| ローカル | Excel: シート名一覧（`excel_sheets`） | [local-files](guides/local-files.md) |
| ローカル | フォルダのファイル一覧（`folder`） | [local-files](guides/local-files.md) |
| ローカル | IME辞書: Microsoft IME（`ime_msime`） | [local-files](guides/local-files.md) |
| ローカル | IME辞書: Google 日本語入力（`ime_google`） | [local-files](guides/local-files.md) |
| クラウド | Notion データベース（`notion`） | [notion](guides/notion.md) |
| Microsoft 365 | Entra ID ユーザー一覧（`graph_entra_users`） | [microsoft365](guides/microsoft365.md) |
| Microsoft 365 | Entra ID グループ一覧（`graph_entra_groups`） | [microsoft365](guides/microsoft365.md) |
| Microsoft 365 | Teams チーム一覧（`graph_teams`） | [microsoft365](guides/microsoft365.md) |
| Microsoft 365 | SharePoint リスト（`graph_sharepoint_list`） | [microsoft365](guides/microsoft365.md) |
| Microsoft 365 | SharePoint ドキュメントライブラリ（`graph_sharepoint_library`） | [microsoft365](guides/microsoft365.md) |
| Microsoft 365 | Entra ID デバイス一覧（`graph_entra_devices`） | [microsoft365](guides/microsoft365.md) |
| Microsoft 365 | Intune デバイス一覧（`graph_intune_devices`） | [microsoft365](guides/microsoft365.md) |
| Microsoft 365 | Microsoft 365 ライセンス一覧（`graph_licenses`） | [microsoft365](guides/microsoft365.md) |
| Microsoft 365 | Outlook 予定表（`graph_calendar`） | [microsoft365](guides/microsoft365.md) |
| Microsoft 365 | OneDrive（組織）（`graph_onedrive`） | [microsoft365](guides/microsoft365.md) |
| Microsoft 365 | OneDrive（個人）（`microsoft_account_onedrive`） | [microsoft365-personal](guides/microsoft365-personal.md) |
| Microsoft 365 | Teams タスク & Planner（`graph_planner`） | [microsoft365](guides/microsoft365.md) |
| ローカル | ブラウザ: 閲覧履歴（`browser_history`） | [local-files](guides/local-files.md) |
| ローカル | ブラウザ: ブックマーク（`browser_bookmarks`） | [local-files](guides/local-files.md) |
| ローカル | ブラウザ: ダウンロード履歴（`browser_downloads`） | [local-files](guides/local-files.md) |
| クラウド | Google スプレッドシート（`google_sheets`） | [google-sheets](guides/google-sheets.md) |
| クラウド | Google カレンダー / iCal (.ics)（`ical_calendar`） | [ical](guides/ical.md) |
| クラウド | Airtable（`airtable`） | [airtable](guides/airtable.md) |
| データベース | PostgreSQL（`postgres`） | [postgresql](guides/postgresql.md) |
| データベース | MySQL（`mysql`） | [mysql](guides/mysql.md) |
| クラウド | Box (ファイル一覧)（`box_files`） | [box](guides/box.md) |
| 公開データ | 郵便番号データベース(日本郵便)（`postal_code`） | [gov-data](guides/gov-data.md) |
| 公開データ | 法人番号 全件データ(国税庁)（`corporate_number`） | [gov-data](guides/gov-data.md) |
| Web | WEBサイト(HTML表 / CSSセレクタ / JSON API)（`web_page`） | [web-page](guides/web-page.md) |
| 公開データ | PokéAPI (ポケモン)（`pokeapi`） | [pokeapi](guides/pokeapi.md) |
<!-- source-catalog:end -->

Microsoft Graphには組織用と個人用のトークン種別があり、組織用には2つの認証方式があります。違いと選び方は
[Microsoft Graph 接続方法の選択](guides/microsoft365.md)を参照してください。
個別手順: [アプリケーション認証](guides/microsoft365-entra-app.md) / [組織向け委任 OAuth](guides/microsoft365-entra-user.md) /
[個人向け OneDrive](guides/microsoft365-personal.md)。

## 検証レベル

ローカル形式と変換処理は自動テストで確認しています。外部サービスは認証情報・ネットワーク・契約が必要なため、実際の接続は利用者の環境で確認してください。開発時に利用可能な環境変数がある場合だけ、ignoredのライブテストを明示的に実行します。
