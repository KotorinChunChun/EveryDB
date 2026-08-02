import type { FieldDef } from "../../source-types/forms";

const EXCEL_CELL_REFERENCE = /^[A-Za-z]{1,3}[1-9][0-9]*$/;

/** 必須項目と種別固有の入力規則を満たしているか判定する。 */
export function isConfigIncomplete(type_: string, fields: FieldDef[], config: Record<string, unknown>): boolean {
  const hasMissingRequiredText = fields.some(
    (field) =>
      (field.kind === "text" || field.kind === "password") &&
      !field.optional &&
      !String(config[field.key] ?? "").trim(),
  );
  if (hasMissingRequiredText) return true;

  if (type_ !== "excel_range" && type_ !== "excel_cell") return false;
  for (const [modeKey, cellKey] of [["start_mode", "start_cell"], ["end_mode", "end_cell"]] as const) {
    if (config[modeKey] !== "manual") continue;
    if (!EXCEL_CELL_REFERENCE.test(String(config[cellKey] ?? "").trim())) return true;
  }
  return false;
}

/** 表示列設定を1フィールド分だけ更新する。undefined は「全フィールド表示」を表す。 */
export function withFieldVisibility(
  displayColumns: string[] | undefined,
  availableFields: string[],
  field: string,
  visible: boolean,
): string[] | undefined {
  if (displayColumns === undefined) {
    return visible ? undefined : availableFields.filter((name) => name !== field);
  }
  const next = displayColumns.filter((name) => name !== field);
  if (visible) next.push(field);
  return next;
}