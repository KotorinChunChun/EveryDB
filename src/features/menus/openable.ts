import { SOURCE_TYPE_METADATA, openCapabilityForType } from "../../source-types/registry";

function includesTable(capability: string): boolean {
  return capability === "table" || capability === "table-and-parent";
}

/** テーブル単位の「開く」操作が意味を持つデータソース種別。 */
export const TABLE_OPENABLE_TYPES = new Set(
  SOURCE_TYPE_METADATA.filter((item) => includesTable(item.open)).map((item) => item.id),
);

export function canOpenTable(type: string): boolean {
  return includesTable(openCapabilityForType(type));
}

export function canOpenParent(type: string): boolean {
  const capability = openCapabilityForType(type);
  return capability === "parent" || capability === "table-and-parent";
}