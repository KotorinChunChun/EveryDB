import { invoke } from "../../app/ipc";
import type { SourceInfo } from "../../shared/types";
import { GRAPH_ONE_CLICK_TYPES } from "./connection-kinds";

export interface DuplicateSourceMatch {
  id: number;
  name: string;
}

function str(v: unknown): string {
  return typeof v === "string" ? v.trim() : "";
}

function identityMatches(
  type_: string,
  cfg: Record<string, unknown>,
  otherType: string,
  otherCfg: Record<string, unknown>,
): boolean {
  if (GRAPH_ONE_CLICK_TYPES.includes(type_)) {
    return type_ === otherType;
  }
  if (type_ === "graph_sharepoint_list") {
    return str(cfg.site_url) === str(otherCfg.site_url) && str(cfg.list_name) === str(otherCfg.list_name);
  }
  if (type_ === "graph_sharepoint_library") {
    return str(cfg.site_url) === str(otherCfg.site_url) && str(cfg.library_name) === str(otherCfg.library_name);
  }
  if (type_ === "notion") {
    return str(cfg.database_id) === str(otherCfg.database_id);
  }
  if (type_ === "airtable") {
    return str(cfg.table) === str(otherCfg.table);
  }
  if (type_ === "sqlite" || type_ === "accdb" || type_ === "excel_table") {
    return str(cfg.path) === str(otherCfg.path) && str(cfg.table) === str(otherCfg.table);
  }
  return false;
}

function candidateParentKeys(type_: string, config: Record<string, unknown>): string[] {
  const keys: string[] = [];
  const path = str(config.path);
  if (path) keys.push(path);
  const site = str(config.site_url);
  if (site) keys.push(site);
  if (type_ === "notion") keys.push("Notion");
  if (type_.startsWith("graph")) keys.push("Microsoft 365");
  if (type_ === "airtable") {
    const baseName = str(config.base_name);
    const baseId = str(config.base_id);
    if (baseName) keys.push(`Airtable: ${baseName}`);
    else if (baseId) keys.push(`Airtable (${baseId})`);
    else keys.push("Airtable");
  }
  return keys;
}

/** 同一接続先配下に同じ identity のソースが既にあるか探す。 */
export async function findDuplicateSource(
  type_: string,
  config: Record<string, unknown>,
  sources: SourceInfo[],
): Promise<DuplicateSourceMatch | null> {
  const parentKeys = new Set(candidateParentKeys(type_, config));
  let candidates = sources.filter((s) => s.type_ === type_);
  if (parentKeys.size > 0) {
    candidates = candidates.filter((s) => parentKeys.has(s.parent));
  }
  if (GRAPH_ONE_CLICK_TYPES.includes(type_)) {
    candidates = sources.filter((s) => s.type_ === type_ && s.parent === "Microsoft 365");
  }
  if (candidates.length === 0) return null;

  for (const c of candidates) {
    const detail = await invoke("get_source_detail", { id: c.id }).catch(() => null);
    if (!detail) continue;
    const other = detail.config;
    if (identityMatches(type_, config, detail.type_, other)) {
      return { id: c.id, name: c.name };
    }
  }
  return null;
}

export function duplicateConfirmMessage(name: string): string {
  return `「${name}」は既に登録されています。新規追加せず、既存テーブルを再読込しますか？`;
}
