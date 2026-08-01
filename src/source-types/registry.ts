import type { CredentialScope } from "../shared/types";

export type ParentKind = "path" | "sharepoint" | "notion" | "airtable" | "graph365" | "postgres" | "mysql" | "box" | "none";
export type SourceCategory = "local" | "microsoft" | "cloud" | "database" | "public-data" | "web";

export interface SourceTypeMetadata {
  id: string;
  category: SourceCategory;
  parentKind: ParentKind;
  guide: string;
  credentialScope?: CredentialScope;
}

const local = (id: string): SourceTypeMetadata => ({ id, category: "local", parentKind: "path", guide: "local-files" });
const graph = (id: string, parentKind: ParentKind = "graph365"): SourceTypeMetadata => ({ id, category: "microsoft", parentKind, guide: "microsoft365", credentialScope: "graph" });

export const SOURCE_TYPE_METADATA: readonly SourceTypeMetadata[] = [
  local("csv"), local("sqlite"), local("accdb"), local("excel_table"), local("excel_range"), local("excel_sheets"),
  local("folder"), local("ime_msime"), local("ime_google"), local("browser_history"), local("browser_bookmarks"), local("browser_downloads"),
  { id: "notion", category: "cloud", parentKind: "notion", guide: "notion", credentialScope: "notion" },
  graph("graph_entra_users"), graph("graph_entra_groups"), graph("graph_teams"),
  graph("graph_sharepoint_list", "sharepoint"), graph("graph_sharepoint_library", "sharepoint"),
  graph("graph_entra_devices"), graph("graph_intune_devices"), graph("graph_licenses"), graph("graph_calendar"), graph("graph_onedrive"), graph("graph_planner"),
  { id: "google_sheets", category: "cloud", parentKind: "none", guide: "google-sheets" },
  { id: "ical_calendar", category: "cloud", parentKind: "none", guide: "ical" },
  { id: "airtable", category: "cloud", parentKind: "airtable", guide: "airtable", credentialScope: "airtable" },
  { id: "postgres", category: "database", parentKind: "postgres", guide: "postgresql", credentialScope: "postgres" },
  { id: "mysql", category: "database", parentKind: "mysql", guide: "mysql", credentialScope: "mysql" },
  { id: "box_files", category: "cloud", parentKind: "box", guide: "box", credentialScope: "box" },
  { id: "postal_code", category: "public-data", parentKind: "none", guide: "gov-data" },
  { id: "corporate_number", category: "public-data", parentKind: "none", guide: "gov-data" },
  { id: "web_page", category: "web", parentKind: "none", guide: "web-page" },
  { id: "pokeapi", category: "public-data", parentKind: "none", guide: "pokeapi" },
] as const;

const metadataById = new Map(SOURCE_TYPE_METADATA.map((metadata) => [metadata.id, metadata]));

export function sourceMetadataFor(type: string): SourceTypeMetadata | undefined {
  return metadataById.get(type);
}

export function guideNameForType(type: string): string | null {
  return sourceMetadataFor(type)?.guide ?? null;
}

export function parentKindForType(type: string): ParentKind {
  return sourceMetadataFor(type)?.parentKind ?? "none";
}

export function representativeTypeForCredentialScope(scope: CredentialScope): string {
  const metadata = SOURCE_TYPE_METADATA.find((item) => item.credentialScope === scope);
  if (!metadata) throw new Error(`Unknown credential scope: ${scope}`);
  return metadata.id;
}