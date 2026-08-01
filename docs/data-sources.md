# データソース一覧と接続手順

| 区分 | 種別 | 主な設定・認証 | 手順書 |
| --- | --- | --- | --- |
| ローカル | CSV / SQLite / Access / Excel / フォルダ / IME | ファイルまたはフォルダのパス、必要に応じてテーブル・シート・範囲 | [ローカルファイル](guides/local-files.md) |
| ローカル | ブラウザ履歴 / ブックマーク / ダウンロード履歴 | ブラウザ種別、検出したプロファイルまたはDBファイルのパス | [ローカルファイル](guides/local-files.md) |
| Microsoft 365 | Entra ID / Teams / SharePoint / Intune / ライセンス / Outlook / OneDrive / Planner | テナントID、クライアントID、クライアントシークレット、対象サイト・ユーザー等 | [Microsoft 365](guides/microsoft365.md) |
| クラウド | Notion | Integration Token、データベースID | [Notion](guides/notion.md) |
| クラウド | Airtable | Personal Access Token、Base/Table/View | [Airtable](guides/airtable.md) |
| クラウド | Google スプレッドシート | サービスアカウント鍵ファイル、スプレッドシートID、範囲 | [Google Sheets](guides/google-sheets.md) |
| クラウド | iCal | 公開 `.ics` URL | [iCal](guides/ical.md) |
| クラウド | Box | OAuth 2.0 または Client Credentials、フォルダID | [Box](guides/box.md) |
| DB | PostgreSQL | ホスト、DB、テーブル、ユーザー、パスワード、SSL設定 | [PostgreSQL](guides/postgresql.md) |
| DB | MySQL | ホスト、DB、テーブル、ユーザー、パスワード、TLS設定 | [MySQL](guides/mysql.md) |
| 公開データ | 郵便番号 / 法人番号 | 自動ダウンロード、または手動で取得したzip/csv | [政府公開データ](guides/gov-data.md) |
| Web | Webページ / JSON API | URL、HTML表・CSSセレクタ・JSON APIのいずれかの設定 | [Webページ](guides/web-page.md) |
| 公開データ | PokéAPI | 設定不要 | [PokéAPI](guides/pokeapi.md) |

## 検証レベル

ローカル形式と変換処理は自動テストで確認しています。外部サービスは認証情報・ネットワーク・契約が必要なため、実際の接続は利用者の環境で確認してください。開発時に利用可能な環境変数がある場合だけ、ignoredのライブテストを明示的に実行します。
