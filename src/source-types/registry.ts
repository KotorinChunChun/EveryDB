import sourceTypeManifest from "../../shared/source-types.json";
import type { CredentialScope } from "../shared/types";

export type ParentKind = "path" | "sharepoint" | "notion" | "airtable" | "graph365" | "postgres" | "mysql" | "box" | "none";
export type SourceCategory = "local" | "microsoft" | "cloud" | "database" | "public-data" | "web";
export type BrowseCapability = "notion" | "sqlite" | "accdb" | "excel_table" | "sharepoint" | "airtable";
export type OpenCapability = "none" | "table" | "parent" | "table-and-parent";
export type GraphAuthMethod = "app_only" | "delegated";
export const GRAPH_CAPABILITIES = [
  "sharepoint",
  "calendar_self",
  "onedrive_self",
  "directory_users",
  "directory_groups",
  "directory_devices",
  "intune_devices",
  "licenses",
] as const;
export type GraphCapability = typeof GRAPH_CAPABILITIES[number];

export function isGraphCapability(value: unknown): value is GraphCapability {
  return typeof value === "string" && GRAPH_CAPABILITIES.some((capability) => capability === value);
}

export interface SourceTypeMetadata {
  id: string;
  label: string;
  category: SourceCategory;
  parentKind: ParentKind;
  guide: string;
  credentialScope?: CredentialScope;
  credentialAuthMethods?: GraphAuthMethod[];
  requiredCredentialCapabilities?: GraphCapability[];
  browse: BrowseCapability | null;
  open: OpenCapability;
}

export interface SourceTypeManifestEntry extends SourceTypeMetadata {
  frontend: boolean;
}

export const SOURCE_TYPE_MANIFEST = sourceTypeManifest as unknown as readonly SourceTypeManifestEntry[];
export const SOURCE_TYPE_METADATA: readonly SourceTypeMetadata[] = SOURCE_TYPE_MANIFEST.filter((entry) => entry.frontend);

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

export function browseCapabilityForType(type: string): BrowseCapability | null {
  return sourceMetadataFor(type)?.browse ?? null;
}

export function openCapabilityForType(type: string): OpenCapability {
  return sourceMetadataFor(type)?.open ?? "none";
}

export function graphCredentialRequirementsFor(type: string): {
  authMethods: readonly GraphAuthMethod[];
  capabilities: readonly GraphCapability[];
} | null {
  const metadata = sourceMetadataFor(type);
  if (metadata?.credentialScope !== "graph" || !metadata.credentialAuthMethods) return null;
  return {
    authMethods: metadata.credentialAuthMethods,
    capabilities: metadata.requiredCredentialCapabilities ?? [],
  };
}

export function representativeTypeForCredentialScope(scope: CredentialScope): string {
  const metadata = SOURCE_TYPE_METADATA.find((item) => item.credentialScope === scope);
  if (!metadata) throw new Error(`Unknown credential scope: ${scope}`);
  return metadata.id;
}
