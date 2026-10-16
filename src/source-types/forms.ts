import type { CredentialScope } from "../shared/types";

export interface FieldDef {
  key: string;
  label: string;
  kind: "text" | "checkbox" | "list" | "password" | "select" | "number" | "textarea";
  placeholder?: string;
  default?: boolean;
  optional?: boolean;
  browse?: "file" | "folder"; // 参照ボタンを付ける（ファイル/フォルダ選択ダイアログ）
  options?: { value: string; label: string }[]; // kind: "select" 用
  min?: number; // kind: "number" 用
  /** API 等から取得した値を表示するだけで、利用者には変更させない。 */
  readOnly?: boolean;
  dialogFilters?: { name: string; extensions: string[] }[]; // browse: "file" 用の拡張子フィルタ
  /** API キー・トークンの取得ページ。指定すると入力欄の横に「🔑 取得…」ボタンを出す。 */
  helpUrl?: string;
  /** ブラウザのプロファイル候補（History/Bookmarks/places.sqlite）を自動検出して選ばせる。
   * 値は探すファイルの種別（"history" は閲覧履歴・ダウンロード履歴の両方に使う。Chromium系では同じ
   * History ファイルを共有するため）。同じダイアログ内の "browser" セレクトの現在値を見て検索する。 */
  browserProfile?: "history" | "bookmarks";
  /** Excelファイル内のシート名一覧を取得して選ばせるボタンを出す。 */
  browseSheet?: boolean;
  /** Excelファイル内のテーブル(ListObject)名一覧を取得して選ばせるボタンを出す。 */
  browseTable?: boolean;
  /** Excelシートの左上/右下の10x10セルを表形式で表示して選択するボタンを出す。 */
  browseCell?: boolean | "top" | "bottom" | "bottom_left" | "bottom_right";
  /** Excelシートの左上/右下の10x10セルを表形式で表示して基準列を選択するボタンを出す。 */
  browseColumn?: boolean | "top" | "bottom" | "bottom_left" | "bottom_right";
  /** 外部ページへの任意リンク。指定すると入力欄の横にリンクボタンを出す。 */
  linkUrl?: string;
  linkLabel?: string;
  /** Airtable の Base 一覧を取得して選ばせるボタンを出す。 */
  browseAirtableBase?: boolean;
  /** Airtable の Table 一覧を取得して選ばせるボタンを出す。 */
  browseAirtableTable?: boolean;
  /** Airtable の View 一覧を取得して選ばせるボタンを出す。 */
  browseAirtableView?: boolean;
  /** この秘密フィールドを保存済みトークンとして再利用できるようにする（要件: トークン管理画面）。
   * 指定すると、秘密値の代わりに保存済みトークンプロファイルを選択するUIを表示する。 */
  credentialScope?: CredentialScope;
  /** 同じトークンを選んだときに一緒に埋めたい非秘密フィールドのキー（tenant_id 等）。
   * 選択時の一回限りのコピーで、以後の「置換」には追随しない（§7-1 参照）。 */
  credentialBundleFields?: string[];
}
export interface TypeDef {
  value: string;
  label: string;
  fields: FieldDef[];
  browse?: "notion" | "sqlite" | "accdb" | "excel_table" | "sharepoint" | "airtable";
}

/** ファイルパス入力欄。browse時、種別に応じた拡張子でファイル選択ダイアログを絞り込む。 */
function pathField(filters?: { name: string; extensions: string[] }[]): FieldDef {
  return { key: "path", label: "ファイルパス", kind: "text", placeholder: "C:\\path\\to\\file", browse: "file", dialogFilters: filters };
}
const EXCEL_FILTERS = [{ name: "Excel ブック", extensions: ["xlsx", "xlsm", "xls"] }];
const CSV_FILTERS = [{ name: "CSV ファイル", extensions: ["csv"] }];
const SQLITE_FILTERS = [{ name: "SQLite データベース", extensions: ["sqlite", "sqlite3", "db", "db3"] }];
const ACCDB_FILTERS = [{ name: "Access データベース", extensions: ["accdb", "mdb"] }];
const TEXT_FILTERS = [{ name: "テキストファイル", extensions: ["txt"] }];
const JSON_FILTERS = [{ name: "JSON ファイル", extensions: ["json"] }];
/** API キー・トークンの取得ページ（入力欄の「🔑 取得…」ボタンから開く）。 */
export const ENTRA_APPS_URL = "https://entra.microsoft.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade";
export const NOTION_INTEGRATIONS_URL = "https://www.notion.so/my-integrations";

/** ブラウザ選択の共通選択肢（要件: 対応ブラウザに Vivaldi を追加。Chromium系なので既存の chrome 等と同じ扱い）。 */
const BROWSER_OPTIONS = [
  { value: "chrome", label: "Google Chrome" }, { value: "edge", label: "Microsoft Edge" },
  { value: "brave", label: "Brave" }, { value: "vivaldi", label: "Vivaldi" }, { value: "firefox", label: "Firefox" },
];

/** 法人番号 全件データの都道府県選択肢。ラベルは国税庁サイトの表示文字列と完全一致させる必要がある
 * （Rust 側が HTML から `<dt class="mb05">県名</dt>` を文字列一致で探すため）。 */
const PREFECTURE_OPTIONS = [
  "北海道", "青森県", "岩手県", "宮城県", "秋田県", "山形県", "福島県", "茨城県", "栃木県", "群馬県",
  "埼玉県", "千葉県", "東京都", "神奈川県", "新潟県", "富山県", "石川県", "福井県", "山梨県", "長野県",
  "岐阜県", "静岡県", "愛知県", "三重県", "滋賀県", "京都府", "大阪府", "兵庫県", "奈良県", "和歌山県",
  "鳥取県", "島根県", "岡山県", "広島県", "山口県", "徳島県", "香川県", "愛媛県", "高知県", "福岡県",
  "佐賀県", "長崎県", "熊本県", "大分県", "宮崎県", "鹿児島県", "沖縄県", "全国",
].map((name) => ({ value: name, label: name }));

/** 政府公開データ（郵便番号・法人番号）に共通の「取得元」フィールド。既定は自動ダウンロード、
 * サイト構造の変更などに備えて手動でダウンロード済みの zip/csv を指定する経路も用意する。 */
const GOV_DATA_SOURCE_FIELDS: FieldDef[] = [
  { key: "source", label: "取得元", kind: "select", options: [
    { value: "auto", label: "自動ダウンロード(既定)" },
    { value: "file", label: "ファイル指定(手動でダウンロード済みの zip/csv)" },
  ] },
  {
    key: "path", label: "ファイル指定時のパス(zip または csv)", kind: "text", optional: true, browse: "file",
    placeholder: "取得元が『ファイル指定』のときのみ使用します",
    dialogFilters: [{ name: "zip / csv", extensions: ["zip", "csv"] }],
  },
];

const GRAPH_AUTH_FIELDS: FieldDef[] = [
  { key: "tenant_id", label: "テナントID", kind: "text", placeholder: "ディレクトリ(テナント) ID", helpUrl: ENTRA_APPS_URL },
  { key: "client_id", label: "クライアントID", kind: "text", placeholder: "アプリケーション(クライアント) ID", helpUrl: ENTRA_APPS_URL },
  { key: "client_secret", label: "クライアントシークレット(値)", kind: "password", placeholder: "シークレットの値(ID ではない)", helpUrl: ENTRA_APPS_URL, credentialScope: "graph", credentialBundleFields: ["tenant_id", "client_id"] },
];
const MICROSOFT_ACCOUNT_AUTH_FIELDS: FieldDef[] = [
  {
    key: "access_token",
    label: "Microsoft アカウント",
    kind: "password",
    credentialScope: "microsoft_account",
  },
];
export const TYPE_DEFS: TypeDef[] = [
  { value: "csv", label: "CSV ファイル", fields: [
    pathField(CSV_FILTERS),
    { key: "encoding", label: "文字コード", kind: "select", options: [
      { value: "auto", label: "自動判定" }, { value: "utf8", label: "UTF-8" }, { value: "shift_jis", label: "Shift_JIS" },
    ] },
    { key: "line_ending", label: "改行コード", kind: "select", options: [
      { value: "auto", label: "自動判定" }, { value: "crlf", label: "CRLF (Windows)" }, { value: "lf", label: "LF" }, { value: "cr", label: "CR" },
    ] },
    { key: "delimiter", label: "区切り文字", kind: "select", options: [
      { value: ",", label: "カンマ (,)" }, { value: "\t", label: "タブ" }, { value: ";", label: "セミコロン (;)" }, { value: "|", label: "パイプ (|)" },
    ] },
    { key: "skip_rows_top", label: "先頭から読み飛ばす行数", kind: "number", placeholder: "0", min: 0, optional: true },
    { key: "has_header", label: "（読み飛ばした後の）先頭行はヘッダー", kind: "checkbox", default: true },
    { key: "skip_rows_bottom", label: "末尾から読み飛ばす行数", kind: "number", placeholder: "0", min: 0, optional: true },
  ] },
  { value: "sqlite", label: "SQLite データベース", fields: [pathField(SQLITE_FILTERS), { key: "table", label: "テーブル名", kind: "text", placeholder: "customers" }], browse: "sqlite" },
  { value: "accdb", label: "Access (accdb / mdb)", fields: [pathField(ACCDB_FILTERS), { key: "table", label: "テーブル名", kind: "text", placeholder: "取引先" }], browse: "accdb" },
  { value: "excel_table", label: "Excel: テーブル(ListObject)", fields: [pathField(EXCEL_FILTERS), { key: "table", label: "テーブル名(ListObject)", kind: "text", placeholder: "テーブル1", browseTable: true }], browse: "excel_table" },
  { value: "excel_range", label: "Excel: シート(セル範囲)", fields: [
    pathField(EXCEL_FILTERS),
    { key: "sheet", label: "シート名", kind: "text", browseSheet: true },
    { key: "start_mode", label: "先頭セル", kind: "select", options: [
      { value: "auto", label: "自動取得" }, { value: "manual", label: "手動指定" },
    ] },
    { key: "start_cell", label: "先頭セル (A1形式)", kind: "text", placeholder: "A1", optional: true, browseCell: "top" },
    { key: "start_ref_column", label: "基準列 (複数可: コロン/カンマ区切り)", kind: "text", placeholder: "A:C, E", optional: true, browseColumn: "top" },
    { key: "end_mode", label: "末尾セル", kind: "select", options: [
      { value: "auto", label: "自動取得" }, { value: "manual", label: "手動指定" },
    ] },
    { key: "end_cell", label: "末尾セル (A1形式)", kind: "text", placeholder: "Z100", optional: true, browseCell: "bottom_right" },
    { key: "end_ref_column", label: "基準列 (複数可: コロン/カンマ区切り)", kind: "text", placeholder: "A:C, E", optional: true, browseColumn: "bottom_left" },
    { key: "has_header", label: "範囲の先頭行はヘッダー", kind: "checkbox", default: true },
  ] },
  { value: "excel_sheets", label: "Excel: シート名一覧", fields: [pathField(EXCEL_FILTERS)] },
  { value: "folder", label: "フォルダのファイル一覧", fields: [{ key: "path", label: "フォルダパス", kind: "text", placeholder: "C:\\path\\to\\folder", browse: "folder" }, { key: "recursive", label: "サブフォルダも含める", kind: "checkbox", default: true }, { key: "extensions", label: "拡張子フィルタ (カンマ区切り・任意)", kind: "list", placeholder: "csv, xlsx" }] },
  { value: "ime_msime", label: "IME辞書: Microsoft IME", fields: [pathField(TEXT_FILTERS)] },
  { value: "ime_google", label: "IME辞書: Google 日本語入力", fields: [pathField(TEXT_FILTERS)] },
  { value: "notion", label: "Notion データベース", fields: [
    { key: "token", label: "インテグレーショントークン", kind: "password", placeholder: "ntn_...", helpUrl: NOTION_INTEGRATIONS_URL, credentialScope: "notion" },
    { key: "url_helper", label: "URL から自動設定", kind: "text", optional: true },
    { key: "database_id", label: "データベースID", kind: "text", placeholder: "URL 内の 32 桁 ID" },
    { key: "database_name", label: "データベース名 (Notionから自動同期)", kind: "text", placeholder: "IDから自動取得します", readOnly: true },
  ], browse: "notion" },
  { value: "graph_entra_users", label: "Entra ID ユーザー一覧", fields: [...GRAPH_AUTH_FIELDS] },
  { value: "graph_entra_groups", label: "Entra ID グループ一覧", fields: [...GRAPH_AUTH_FIELDS] },
  { value: "graph_teams", label: "Teams チーム一覧", fields: [...GRAPH_AUTH_FIELDS] },
  { value: "graph_sharepoint_list", label: "SharePoint リスト", fields: [
    ...GRAPH_AUTH_FIELDS,
    { key: "url_helper", label: "URL から自動設定", kind: "text", optional: true },
    { key: "site_url", label: "サイトURL", kind: "text", placeholder: "https://～/sites/サイト名" },
    { key: "site_display_name", label: "サイト表示名 (自動)", kind: "text", optional: true },
    { key: "list_name", label: "リスト名", kind: "text", placeholder: "SampleList" },
  ], browse: "sharepoint" },
  { value: "graph_sharepoint_library", label: "SharePoint ドキュメントライブラリ", fields: [
    ...GRAPH_AUTH_FIELDS,
    { key: "url_helper", label: "URL から自動設定", kind: "text", optional: true },
    { key: "site_url", label: "サイトURL", kind: "text", placeholder: "https://～/sites/サイト名" },
    { key: "site_display_name", label: "サイト表示名 (自動)", kind: "text", optional: true },
    { key: "library_name", label: "ライブラリ名(空欄=既定)", kind: "text", placeholder: "空欄で既定ライブラリ", optional: true },
  ], browse: "sharepoint" },
  { value: "graph_entra_devices", label: "Entra ID デバイス一覧", fields: [...GRAPH_AUTH_FIELDS] },
  { value: "graph_intune_devices", label: "Intune デバイス一覧", fields: [...GRAPH_AUTH_FIELDS] },
  { value: "graph_licenses", label: "Microsoft 365 ライセンス一覧", fields: [...GRAPH_AUTH_FIELDS] },
  { value: "graph_calendar", label: "Outlook 予定表", fields: [...GRAPH_AUTH_FIELDS, { key: "user", label: "対象ユーザー(UPN/メール)", kind: "text", placeholder: "user@example.com" }] },
  { value: "graph_onedrive", label: "OneDrive（組織）", fields: [...GRAPH_AUTH_FIELDS, { key: "user", label: "対象ユーザー(UPN/メール)", kind: "text", placeholder: "user@example.com" }] },
  { value: "microsoft_account_onedrive", label: "OneDrive（個人）", fields: [...MICROSOFT_ACCOUNT_AUTH_FIELDS] },
  { value: "graph_planner", label: "Teams タスク & Planner", fields: [...GRAPH_AUTH_FIELDS, { key: "group_id", label: "対象チーム(グループ)のID", kind: "text", placeholder: "Entra管理センター等で確認したグループID" }] },
  { value: "browser_history", label: "ブラウザ: 閲覧履歴", fields: [
    { key: "browser", label: "ブラウザ", kind: "select", options: BROWSER_OPTIONS },
    { key: "path", label: "プロファイル", kind: "text", placeholder: "「候補から選択…」で自動検出できます", browse: "file", browserProfile: "history" },
  ] },
  { value: "browser_bookmarks", label: "ブラウザ: ブックマーク", fields: [
    { key: "browser", label: "ブラウザ", kind: "select", options: BROWSER_OPTIONS },
    { key: "path", label: "プロファイル", kind: "text", placeholder: "「候補から選択…」で自動検出できます", browse: "file", browserProfile: "bookmarks" },
  ] },
  { value: "browser_downloads", label: "ブラウザ: ダウンロード履歴", fields: [
    { key: "browser", label: "ブラウザ", kind: "select", options: BROWSER_OPTIONS },
    { key: "path", label: "プロファイル", kind: "text", placeholder: "「候補から選択…」で自動検出できます", browse: "file", browserProfile: "history" },
  ] },
  { value: "google_sheets", label: "Google スプレッドシート", fields: [
    { key: "key_path", label: "サービスアカウント鍵(JSON)のパス", kind: "text", placeholder: "C:\\path\\to\\service-account.json", browse: "file", dialogFilters: JSON_FILTERS, helpUrl: "https://console.cloud.google.com/apis/credentials" },
    { key: "url_helper", label: "URL から自動設定", kind: "text", optional: true },
    { key: "spreadsheet_id", label: "スプレッドシートID", kind: "text", placeholder: "URL の /d/ と /edit の間の文字列" },
    { key: "range", label: "範囲(任意)", kind: "text", placeholder: "Sheet1!A1:D50 / 空欄=先頭シート全域", optional: true },
    { key: "has_header", label: "先頭行はヘッダー", kind: "checkbox", default: true },
  ] },
  { value: "ical_calendar", label: "Google カレンダー / iCal (.ics)", fields: [
    { key: "url", label: "公開 iCal URL", kind: "text", placeholder: "https://calendar.google.com/calendar/ical/.../public/basic.ics" },
  ] },
  { value: "airtable", label: "Airtable", fields: [
    { key: "token", label: "Personal Access Token", kind: "password", placeholder: "pat...", helpUrl: "https://airtable.com/create/tokens", linkUrl: "https://airtable.com/workspaces", linkLabel: "🌐 ワークスペース…", credentialScope: "airtable" },
    { key: "url_helper", label: "URL から自動設定", kind: "text", optional: true },
    { key: "base_id", label: "Base ID (実際の接続用)", kind: "text", placeholder: "app...", browseAirtableBase: true },
    { key: "base_name", label: "Base名 (キャプション・確認用)", kind: "text", placeholder: "選択時に自動補完されます", optional: true },
    { key: "table", label: "テーブル ID (実際の接続用)", kind: "text", placeholder: "tbl... または テーブル名", browseAirtableTable: true },
    { key: "table_name", label: "テーブル名 (キャプション・確認用)", kind: "text", placeholder: "選択時に自動補完されます", optional: true },
    { key: "view", label: "ビュー ID (実際の接続用・任意)", kind: "text", placeholder: "viw... または ビュー名", optional: true, browseAirtableView: true },
    { key: "view_name", label: "ビュー名 (キャプション・確認用)", kind: "text", placeholder: "選択時に自動補完されます", optional: true },
  ], browse: "airtable" },
  { value: "postgres", label: "PostgreSQL", fields: [
    { key: "host", label: "ホスト", kind: "text", placeholder: "localhost / xxx.rds.amazonaws.com" },
    { key: "port", label: "ポート", kind: "text", placeholder: "5432", optional: true },
    { key: "database", label: "データベース名", kind: "text" },
    { key: "schema", label: "スキーマ(任意)", kind: "text", placeholder: "public", optional: true },
    { key: "table", label: "テーブル名", kind: "text" },
    { key: "user", label: "ユーザー", kind: "text" },
    { key: "password", label: "パスワード", kind: "password", credentialScope: "postgres", credentialBundleFields: ["user"] },
    { key: "sslmode", label: "SSLモード(任意)", kind: "select", optional: true, options: [
      { value: "prefer", label: "prefer (可能なら暗号化・既定)" },
      { value: "require", label: "require (暗号化を必須)" },
      { value: "verify-full", label: "verify-full (証明書とホスト名を検証)" },
      { value: "disable", label: "disable (暗号化しない・非推奨)" },
    ] },
    { key: "ssl_accept_invalid_certs", label: "自己署名証明書を許可(社内サーバー向け)", kind: "checkbox", default: false },
  ] },
  { value: "mysql", label: "MySQL", fields: [
    { key: "host", label: "ホスト", kind: "text", placeholder: "localhost" },
    { key: "port", label: "ポート", kind: "text", placeholder: "3306", optional: true },
    { key: "database", label: "データベース名", kind: "text" },
    { key: "table", label: "テーブル名", kind: "text" },
    { key: "user", label: "ユーザー", kind: "text" },
    { key: "password", label: "パスワード", kind: "password", credentialScope: "mysql", credentialBundleFields: ["user"] },
    { key: "use_tls", label: "TLS で接続する(リモート接続時は必ず有効化)", kind: "checkbox", default: false },
    { key: "ssl_accept_invalid_certs", label: "自己署名証明書を許可(社内サーバー向け)", kind: "checkbox", default: false },
  ] },
  { value: "box_files", label: "Box (ファイル一覧)", fields: [
    { key: "access_token", label: "使用するトークン", kind: "password", credentialScope: "box" },
    { key: "folder_id", label: "開始フォルダID(任意)", kind: "text", placeholder: "0 (ルート)", optional: true },
  ] },
  { value: "postal_code", label: "郵便番号データベース(日本郵便)", fields: [...GOV_DATA_SOURCE_FIELDS] },
  { value: "corporate_number", label: "法人番号 全件データ(国税庁)", fields: [
    { key: "prefecture", label: "都道府県", kind: "select", options: PREFECTURE_OPTIONS },
    ...GOV_DATA_SOURCE_FIELDS,
  ] },
  { value: "web_page", label: "WEBサイト(HTML表 / CSSセレクタ / JSON API)", fields: [
    { key: "url", label: "URL", kind: "text", placeholder: "https://example.com/search?q=... （GETクエリ込みで指定）" },
    { key: "mode", label: "取り込み方式", kind: "select", options: [
      { value: "html_table", label: "HTML表(table)を自動取込" },
      { value: "css_selector", label: "CSSセレクタで行/列を指定" },
      { value: "json_api", label: "JSON APIを取込" },
    ] },
    { key: "table_index", label: "何番目の<table>か（html_table用、既定0）", kind: "number", min: 0, optional: true, placeholder: "0" },
    { key: "has_header", label: "先頭行を見出しにする（html_table用）", kind: "checkbox", default: true },
    { key: "row_selector", label: "行のCSSセレクタ（css_selector用）", kind: "text", optional: true, placeholder: "例: article.post" },
    {
      key: "columns", label: "列定義（css_selector・json_api用。1行に1列）", kind: "textarea", optional: true,
      placeholder: "css_selector例:\nタイトル=h2 a\nURL=h2 a@href\n\njson_api例:\nタイトル=title\nいいね=liked_count",
    },
    { key: "json_root", label: "配列を取り出すJSONパス（json_api用、空欄=レスポンス自体が配列）", kind: "text", optional: true, placeholder: "例: articles" },
  ] },
  { value: "pokeapi", label: "PokéAPI (ポケモン)", fields: [] },
];
