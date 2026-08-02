/** テーブル単位の「開く」操作が意味を持つデータソース種別。 */
export const TABLE_OPENABLE_TYPES = new Set([
  "notion", "graph_entra_users", "graph_entra_groups", "graph_entra_devices",
  "graph_intune_devices", "graph_licenses", "graph_teams",
  "graph_sharepoint_list", "graph_sharepoint_library",
  "airtable", "google_sheets", "ical_calendar",
]);

export function canOpenTable(type: string): boolean {
  return TABLE_OPENABLE_TYPES.has(type);
}
