import sourceTypeManifest from "../../shared/source-types.json";
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

export function representativeTypeForCredentialScope(scope: CredentialScope): string {
  const metadata = SOURCE_TYPE_METADATA.find((item) => item.credentialScope === scope);
  if (!metadata) throw new Error(`Unknown credential scope: ${scope}`);
  return metadata.id;
}