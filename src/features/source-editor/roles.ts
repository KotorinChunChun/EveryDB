/** フィールド役割エディタの編集中状態。 */
export interface RoleState {
  label_column?: string;
  open_ref_column?: string;
  searchable: Set<string>;
  display: Set<string>;
  aliases: Record<string, string>;
  order?: string[];
}

const ROLE_CONFIG_KEYS = [
  "label_column",
  "open_ref_column",
  "searchable_columns",
  "display_columns",
  "column_aliases",
  "column_order",
] as const;

/** 保存済みの並び順(order)を先頭に、そこに無い新規フィールドは検出順のまま末尾へ続ける。 */
export function applyColumnOrder(names: readonly string[], order?: readonly string[]): string[] {
  if (!order || order.length === 0) return [...names];
  const known = order.filter((name) => names.includes(name));
  const extra = names.filter((name) => !order.includes(name));
  return [...known, ...extra];
}

/** ソース config から RoleState を構築する。 */
export function roleStateFromConfig(cfg: Record<string, unknown>): RoleState {
  return {
    label_column: cfg.label_column as string | undefined,
    open_ref_column: cfg.open_ref_column as string | undefined,
    searchable: new Set((cfg.searchable_columns as string[] | undefined) ?? []),
    display: new Set((cfg.display_columns as string[] | undefined) ?? []),
    aliases: (cfg.column_aliases as Record<string, string> | undefined) ?? {},
    order: cfg.column_order as string[] | undefined,
  };
}

/** config から役割系キーを除いたコピーを返す。 */
export function configWithoutRoleKeys(cfg: Record<string, unknown>): Record<string, unknown> {
  const next = { ...cfg };
  for (const key of ROLE_CONFIG_KEYS) delete next[key];
  return next;
}

/** 役割テーブル DOM から保存用 config 断片を収集する。 */
export function collectRoleConfigFromTable(
  rowsEl: HTMLTableSectionElement,
  fieldNames: readonly string[],
): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  if (fieldNames.length === 0) return out;

  const displayCols: string[] = [];
  const searchableCols: string[] = [];
  const aliases: Record<string, string> = {};
  let labelCol: string | undefined;
  let openRefCol: string | undefined;

  for (const row of Array.from(rowsEl.querySelectorAll("tr"))) {
    const name = (row as HTMLElement).dataset.name!;
    if ((row.querySelector(".r-display") as HTMLInputElement).checked) displayCols.push(name);
    if ((row.querySelector(".r-search") as HTMLInputElement).checked) searchableCols.push(name);
    const labelRadio = row.querySelector('input[name="r-label"]') as HTMLInputElement;
    if (labelRadio.checked) labelCol = name;
    const openRefRadio = row.querySelector('input[name="r-openref"]') as HTMLInputElement;
    if (openRefRadio.checked) openRefCol = name;
    const alias = (row.querySelector(".r-alias") as HTMLInputElement).value.trim();
    if (alias && alias !== name) aliases[name] = alias;
  }

  out.display_columns = displayCols;
  out.searchable_columns = searchableCols;
  if (labelCol) out.label_column = labelCol;
  if (openRefCol) out.open_ref_column = openRefCol;
  if (Object.keys(aliases).length) out.column_aliases = aliases;
  out.column_order = Array.from(rowsEl.querySelectorAll("tr")).map(
    (row) => (row as HTMLElement).dataset.name!,
  );
  return out;
}
