import { TYPE_DEFS } from "../../source-types/forms";

/** ファイルパス両端のダブルクォート・空白を除去して、表示用の末尾名を返す。 */
export function basenameOf(path: string): string {
  let normalized = path.trim();
  if (normalized.length >= 2 && normalized.startsWith('"') && normalized.endsWith('"')) {
    normalized = normalized.slice(1, -1).trim();
  }
  const parts = normalized.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] || normalized;
}

/** Notionの最新名で置換してよいのは、空・以前の自動名・旧ID短縮名・種別既定名だけ。 */
export function shouldUpdateNotionDisplayName(current: string, previousDatabaseName: string, databaseId: string): boolean {
  const value = current.trim();
  return !value
    || value === previousDatabaseName.trim()
    || value === `Notion DB (${databaseId.slice(0, 8)}...)`
    || value === "Notion データベース";
}
/** データソース種別と接続設定値から、推奨される表示名を導出する。 */
export function deriveDefaultName(type_: string, config: Record<string, unknown>): string {
  const str = (key: string): string => String(config[key] ?? "").trim();
  const base = basenameOf;

  switch (type_) {
    case "csv":
    case "sqlite":
    case "accdb":
    case "folder":
    case "ime_msime":
    case "ime_google":
    case "browser_history":
    case "browser_bookmarks":
    case "browser_downloads": {
      const p = str("path");
      if (p) return base(p);
      break;
    }
    case "excel_table": {
      const p = str("path");
      const t = str("table");
      if (p && t) return `${base(p)}: ${t}`;
      if (p) return base(p);
      break;
    }
    case "excel_range": {
      const p = str("path");
      const s = str("sheet");
      const r = str("range"); // 旧フォーマット互換
      const sc = str("start_cell");
      const ec = str("end_cell");
      const rangeStr = r || (sc && ec ? `${sc}:${ec}` : sc || "");
      if (p && s && rangeStr) return `${base(p)}: ${s}!${rangeStr}`;
      if (p && s) return `${base(p)}: ${s}`;
      if (p) return base(p);
      break;
    }
    case "excel_sheets": {
      const p = str("path");
      if (p) return `${base(p)} (シート一覧)`;
      break;
    }
    case "web_page":
    case "ical_calendar": {
      const u = str("url");
      if (u) {
        try {
          const parsed = new URL(u);
          const p = parsed.pathname.split("/").filter(Boolean);
          const last = p[p.length - 1];
          return last ? `${parsed.hostname}/${last}` : parsed.hostname;
        } catch {
          return base(u);
        }
      }
      break;
    }
    case "graph_sharepoint_list": {
      const l = str("list_name");
      if (l) return l;
      const site = str("site_display_name");
      if (site) return site;
      break;
    }
    case "graph_sharepoint_library": {
      const l = str("library_name");
      if (l) return l;
      const site = str("site_display_name");
      if (site) return site;
      return "ドキュメントライブラリ";
    }
    case "postgres":
    case "mysql": {
      const db = str("database");
      const tbl = str("table");
      if (db && tbl) return `${db}.${tbl}`;
      if (tbl) return tbl;
      if (db) return db;
      break;
    }
    case "airtable": {
      const tblName = str("table_name");
      const tbl = str("table");
      const viwName = str("view_name");
      const viw = str("view");
      const baseName = str("base_name");
      const t = tblName || tbl;
      const v = viwName || viw;
      if (t && v) return baseName ? `${baseName}: ${t} (${v})` : `${t} (${v})`;
      if (t) return baseName ? `${baseName}: ${t}` : t;
      if (baseName) return baseName;
      break;
    }
    case "google_sheets": {
      const sid = str("spreadsheet_id");
      const r = str("range");
      if (sid && r) return `Sheet (${r})`;
      if (sid) return `Sheet (${sid.slice(0, 8)}...)`;
      break;
    }
    case "notion": {
      const databaseName = str("database_name");
      if (databaseName) return databaseName;
      const did = str("database_id");
      if (did) return `Notion DB (${did.slice(0, 8)}...)`;
      break;
    }
  }

  // 接続設定からの導出に該当しない場合やクラウドAPI系は、種別ラベルをデフォルト値として返す
  const def = TYPE_DEFS.find((t) => t.value === type_);
  return def?.label ?? type_;
}

