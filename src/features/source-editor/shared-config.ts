import type { ParentKind } from "../../source-types/registry";
import { TYPE_DEFS } from "../../source-types/forms";
import { SECRET_PLACEHOLDER } from "./constants";

/** 接続先(接続設定)ダイアログに表示する、接続種別の閲覧用ラベル。 */
export function parentKindLabel(kind: ParentKind, repType: string): string {
  switch (kind) {
    case "path": return `ファイル（${TYPE_DEFS.find((t) => t.value === repType)?.label ?? repType}）`;
    case "sharepoint": return "SharePoint サイト";
    case "notion": return "Notion";
    case "airtable": return "Airtable";
    case "graph365": return "Microsoft 365 (Graph)";
    case "postgres": return "PostgreSQL サーバー";
    case "mysql": return "MySQL サーバー";
    case "box": return "Box";
    default: return repType;
  }
}

/** その接続先種別で「接続情報」として接続先側にまとめる config キー。 */
export function sharedKeysFor(kind: ParentKind): string[] {
  switch (kind) {
    case "path": return ["path"];
    case "sharepoint": return ["site_url", "tenant_id", "client_id", "client_secret"];
    case "notion": return ["token"];
    case "airtable": return ["token"];
    case "graph365": return ["tenant_id", "client_id", "client_secret"];
    case "postgres": return ["host", "port", "database", "user", "password"];
    case "mysql": return ["host", "port", "database", "user", "password"];
    case "box": return ["access_token"];
    default: return [];
  }
}

/** フィールド入力欄の「現在の実効値」。 */
export function currentFieldRawValue(
  el: HTMLInputElement,
  key: string,
  normalizePath: (value: string | undefined | null) => string,
): string {
  if (el.dataset.credRef && el.value === SECRET_PLACEHOLDER) return `cred:${el.dataset.credRef}`;
  return key === "path" ? normalizePath(el.value) : el.value.trim();
}
