export interface SourceInfo {
  id: number;
  type_: string;
  name: string;
  refresh_interval: number;
  status: string | null;
  last_success_at: string | null;
  last_error: string | null;
  record_count: number;
  parent: string;
}

export interface Row {
  source_id: number;
  source_name: string;
  label: string | null;
  open_ref: string | null;
  row_number?: number | null;
  fields: Record<string, string>;
}

export interface SearchResponse {
  seq: number;
  rows: Row[];
  truncated: boolean;
}

export interface WinAuthStatus {
  available: boolean;
  unlocked: boolean;
}

export interface CredentialLink {
  id: number;
  label: string;
}

export interface SourceDetail {
  type_: string;
  name: string;
  refresh_interval: number;
  config: Record<string, unknown>;
  unlocked: boolean;
  has_secret: boolean;
  linked_credentials: Record<string, CredentialLink>;
}

export type CredentialScope = "notion" | "airtable" | "graph" | "microsoft_account" | "box" | "postgres" | "mysql";

export interface CredentialUsage {
  source_id: number;
  source_name: string;
}

export interface CredentialSummary {
  id: number;
  scope: string;
  label: string;
  created_at: string;
  used_by: CredentialUsage[];
}
export type ReloadOutcome = "success" | "failed" | "cancelled";

export interface ReloadSourceResult {
  source_id: number;
  outcome: ReloadOutcome;
  record_count: number | null;
  error: string | null;
}

export interface ReloadSummary {
  successful: number;
  failed: number;
  cancelled: number;
  results: ReloadSourceResult[];
}
