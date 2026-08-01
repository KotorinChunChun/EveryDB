import type { SourceInfo } from "../../shared/types";
import { TYPE_DEFS } from "../../source-types/forms";

export type SourceTreeEntry =
  | { kind: "source"; source: SourceInfo }
  | { kind: "parent"; parent: string; children: SourceInfo[] };

/** 親接続先の表示名。明示的な別名を優先し、パスは末尾要素だけを表示する。 */
export function parentLabel(parent: string, overrides: ReadonlyMap<string, string>): string {
  const override = overrides.get(parent);
  if (override) return override;
  if (/[\\/]/.test(parent)) {
    const parts = parent.split(/[\\/]/).filter(Boolean);
    return parts[parts.length - 1] || parent;
  }
  return parent;
}

/** 種別キーを利用者向けの表示名に変換する。 */
export function sourceTypeLabel(type: string): string {
  return TYPE_DEFS.find((definition) => definition.value === type)?.label ?? type;
}

/** 最終更新日時を一覧用の短い表示に整形する。 */
export function shortTimestamp(timestamp: string | null): string {
  if (!timestamp) return "―";
  const match = /(\d{4})-(\d{2})-(\d{2})[T ](\d{2}):(\d{2})/.exec(timestamp);
  return match ? `${match[2]}-${match[3]} ${match[4]}:${match[5]}` : timestamp;
}

/** 親接続先とテーブルを、元の並び順を保った描画単位にまとめる。 */
export function sourceTree(sources: readonly SourceInfo[]): SourceTreeEntry[] {
  const childrenByParent = new Map<string, SourceInfo[]>();
  for (const source of sources) {
    if (!source.parent) continue;
    const children = childrenByParent.get(source.parent) ?? [];
    children.push(source);
    childrenByParent.set(source.parent, children);
  }

  const emittedParents = new Set<string>();
  const entries: SourceTreeEntry[] = [];
  for (const source of sources) {
    if (!source.parent) {
      entries.push({ kind: "source", source });
      continue;
    }
    if (emittedParents.has(source.parent)) continue;
    emittedParents.add(source.parent);
    entries.push({ kind: "parent", parent: source.parent, children: childrenByParent.get(source.parent)! });
  }
  return entries;
}

/** テーブル名または親接続先名の部分一致でフィルタする。 */
export function sourceMatchesFilter(
  source: SourceInfo,
  filter: string,
  overrides: ReadonlyMap<string, string>,
): boolean {
  const query = filter.trim().toLowerCase();
  return !query
    || source.name.toLowerCase().includes(query)
    || parentLabel(source.parent, overrides).toLowerCase().includes(query);
}