/** Microsoft 365: 接続情報だけで使える＝ワンクリックで追加できる種別。 */
export const GRAPH_ONE_CLICK_TYPES: string[] = [
  "graph_entra_users", "graph_entra_groups", "graph_entra_devices",
  "graph_intune_devices", "graph_licenses", "graph_teams",
];

export type ParentBrowseKind = "sqlite" | "accdb" | "excel_table" | "sharepoint" | "notion" | "graph365" | "airtable";

/** 接続先ダイアログから配下テーブルを一覧取得して追加/削除できる種別。 */
export function parentBrowseKind(repType: string): ParentBrowseKind | null {
  if (repType === "sqlite") return "sqlite";
  if (repType === "accdb") return "accdb";
  if (repType === "excel_table") return "excel_table";
  if (repType.startsWith("graph_sharepoint")) return "sharepoint";
  if (repType === "notion") return "notion";
  if (repType === "airtable") return "airtable";
  if (repType.startsWith("graph")) return "graph365";
  return null;
}

export interface ConnectionKindOption {
  id: string;
  label: string;
  repType: string;
  /** true = 従来の単体テーブル追加（種別一覧から選択） */
  standalone?: boolean;
}

/** ＋追加 → 接続先種別の最初の選択肢。 */
export const CONNECTION_KIND_OPTIONS: ConnectionKindOption[] = [
  { id: "graph365", label: "Microsoft 365 (Graph)", repType: "graph_entra_users" },
  { id: "sharepoint", label: "SharePoint サイト", repType: "graph_sharepoint_list" },
  { id: "notion", label: "Notion", repType: "notion" },
  { id: "airtable", label: "Airtable", repType: "airtable" },
  { id: "file", label: "SQLite / Access / Excel（ファイル）", repType: "sqlite" },
  { id: "database", label: "PostgreSQL / MySQL / Box", repType: "postgres" },
  { id: "standalone", label: "その他（単体テーブル）", repType: "csv", standalone: true },
];
