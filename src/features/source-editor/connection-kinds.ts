import { SOURCE_TYPE_METADATA, type SourceCategory } from "../../source-types/registry";

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
  category: SourceCategory;
  /** 保存後に、その接続先で利用できるテーブル一覧を開く。 */
  connectionAdd: boolean;
  /** 既存の Microsoft 365 接続があれば、その接続設定を直接開く。 */
  openExistingGraphParent: boolean;
}

export interface ConnectionKindGroup {
  category: SourceCategory;
  label: string;
  options: ConnectionKindOption[];
}

const CONNECTION_CATEGORY_LABELS: ReadonlyArray<{ category: SourceCategory; label: string }> = [
  { category: "microsoft", label: "Microsoft 365 / OneDrive" },
  { category: "local", label: "ローカルファイル・PC" },
  { category: "cloud", label: "クラウドサービス" },
  { category: "database", label: "データベース" },
  { category: "public-data", label: "公開データ" },
  { category: "web", label: "Web" },
];

/**
 * 「接続先を追加」に表示する選択肢。
 * 抽象的な接続先分類ではなく、追加後に作成される全テーブル種別を直接列挙する。
 */
export const CONNECTION_KIND_OPTIONS: ConnectionKindOption[] = SOURCE_TYPE_METADATA.map((metadata) => ({
  id: metadata.id,
  label: metadata.label,
  repType: metadata.id,
  category: metadata.category,
  connectionAdd: parentBrowseKind(metadata.id) !== null,
  // 対象指定が不要な Graph 種別だけは、既存接続の一覧からワンクリックで増減できる。
  // OneDrive / Outlook / Planner は追加設定が必要なので、必ず各種別の追加画面を開く。
  openExistingGraphParent: GRAPH_ONE_CLICK_TYPES.includes(metadata.id),
}));

export const CONNECTION_KIND_GROUPS: ConnectionKindGroup[] = CONNECTION_CATEGORY_LABELS.map(({ category, label }) => ({
  category,
  label,
  options: CONNECTION_KIND_OPTIONS.filter((option) => option.category === category),
})).filter((group) => group.options.length > 0);
