import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type {
  CredentialSummary,
  CredentialUsage,
  ReloadSummary,
  SearchResponse,
  SourceDetail,
  SourceInfo,
  WinAuthStatus,
} from "../shared/types";

export interface SearchPresetPayload {
  id: number;
  name: string;
  query: string;
  source_ids: number[];
  created_at: string;
}

export interface CredentialPublicFields {
  label: string;
  fields: Record<string, unknown>;
}

export interface OAuthTokenResult {
  refresh_token: string;
}


export interface NotionDatabaseItem {
  id: string;
  title: string;
}

export interface AirtableBaseItem {
  id: string;
  name: string;
  permission_level: string;
}

export interface AirtableViewItem {
  id: string;
  name: string;
  type_: string;
}

export interface AirtableTableItem {
  id: string;
  name: string;
  primary_field_id: string;
  views: AirtableViewItem[];
}

export interface GraphSiteItem {
  id: string;
  display_name: string;
  web_url: string;
  description: string;
  created_date_time: string;
}

export interface GraphTeamItem {
  id: string;
  display_name: string;
  description: string;
  created_date_time: string;
}

export interface SiteResourceItem {
  display_name: string;
  kind: string;
}

export interface ExcelGridPreview {
  start_row: number;
  start_col: number;
  rows: string[][];
}

export interface BrowserProfileItem {
  label: string;
  path: string;
}

export interface FieldPreview {
  names: string[];
  from_cache: boolean;
}

/** Rust の invoke_handler と 1 対 1 で同期する IPC 契約。 */
export interface IpcContract {
  list_sources: { args: undefined; result: SourceInfo[] };
  add_source: { args: { type: string; name: string; configJson: string }; result: number };
  add_source_from_sibling: {
    args: { siblingId: number; type: string; name: string; configJson: string; sharedKeys: string[] };
    result: number;
  };
  delete_source: { args: { id: number }; result: void };
  get_source_detail: { args: { id: number }; result: SourceDetail };
  update_source: { args: { id: number; name: string; configJson: string }; result: void };
  duplicate_source: { args: { id: number; newName: string }; result: number };
  reorder_sources: { args: { orderedIds: number[] }; result: void };
  preview_source_fields: { args: { type: string; configJson: string }; result: string[] };
  preview_source_fields_by_id: { args: { id: number }; result: FieldPreview };
  set_refresh_interval: { args: { id: number; seconds: number }; result: void };
  reload_sources: { args: { sourceIds: number[] }; result: ReloadSummary };

  search: {
    args: { query: string; sourceIds: number[]; seq: number; limit?: number | null };
    result: SearchResponse;
  };
  add_search_history_entry: { args: { query: string }; result: void };
  list_search_history: { args: undefined; result: string[] };
  save_search_preset: { args: { name: string; query: string; sourceIds: number[] }; result: number };
  list_search_presets: { args: undefined; result: SearchPresetPayload[] };
  delete_search_preset: { args: { id: number }; result: void };

  open_ref: { args: { target: string }; result: void };
  encode_shift_jis: { args: { text: string }; result: number[] };

  winauth_status: { args: undefined; result: WinAuthStatus };
  winauth_verify: { args: undefined; result: boolean };
  list_credentials: { args: { scope: string }; result: CredentialSummary[] };
  add_credential: { args: { scope: string; label: string; fieldsJson: string }; result: number };
  update_credential: { args: { id: number; label: string; fieldsJson: string }; result: void };
  delete_credential: { args: { id: number }; result: void };
  credential_usage: { args: { id: number }; result: CredentialUsage[] };
  credential_public_fields: { args: { id: number }; result: CredentialPublicFields };
  credential_secret_value: { args: { id: number }; result: string };
  box_oauth_authorize: {
    args: { clientId: string; clientSecret: string };
    result: OAuthTokenResult;
  };
  box_oauth_cancel: { args: undefined; result: void };
  graph_oauth_authorize: {
    args: { tenantId: string; clientId: string; capabilities: string[] };
    result: OAuthTokenResult;
  };
  microsoft_account_oauth_authorize: {
    args: { clientId: string };
    result: OAuthTokenResult;
  };
  graph_oauth_cancel: { args: undefined; result: void };

  browse_notion_databases: { args: { credentialId: number }; result: NotionDatabaseItem[] };
  browse_notion_databases_for_source: {
    args: { id: number; overridesJson: string };
    result: NotionDatabaseItem[];
  };
  browse_airtable_bases: { args: { credentialId: number }; result: AirtableBaseItem[] };
  browse_airtable_bases_for_source: {
    args: { id: number; overridesJson: string };
    result: AirtableBaseItem[];
  };
  browse_airtable_tables: {
    args: { credentialId: number; baseId: string };
    result: AirtableTableItem[];
  };
  browse_airtable_tables_for_source: {
    args: { id: number; overridesJson: string; baseId: string | null };
    result: AirtableTableItem[];
  };
  browse_graph_site_resources_for_source: {
    args: { id: number; overridesJson: string };
    result: SiteResourceItem[];
  };
  browse_graph_sites: { args: { credentialId: number }; result: GraphSiteItem[] };
  browse_graph_teams: { args: { credentialId: number }; result: GraphTeamItem[] };
  browse_graph_team_site_url: {
    args: { credentialId: number; teamGroupId: string };
    result: string;
  };
  browse_graph_site_resources: {
    args: { credentialId: number; siteUrl: string };
    result: SiteResourceItem[];
  };
  browse_excel_sheets: { args: { path: string }; result: string[] };
  browse_excel_tables: { args: { path: string }; result: string[] };
  browse_excel_grid: {
    args: { path: string; sheet: string; rows: number; cols: number; mode?: string | null };
    result: ExcelGridPreview;
  };
  browse_sqlite_tables: { args: { path: string }; result: string[] };
  browse_accdb_tables: { args: { path: string }; result: string[] };
  browse_browser_profiles: {
    args: { browser: string; target: string };
    result: BrowserProfileItem[];
  };

  autostart_status: { args: undefined; result: boolean };
  autostart_set: { args: { enabled: boolean }; result: void };
  set_zoom: { args: { factor: number }; result: void };
  get_zoom: { args: undefined; result: number };
  set_theme: { args: { theme: string }; result: void };
  get_theme: { args: undefined; result: string };
  set_hotkey: { args: { hotkey: string }; result: void };
  get_hotkey: { args: undefined; result: string };
  set_sidebar_width: { args: { width: number }; result: void };
  get_sidebar_width: { args: undefined; result: number };
  list_parent_labels: { args: undefined; result: [string, string][] };
  set_parent_label: { args: { parentKey: string; label: string }; result: void };
}

export type IpcCommand = keyof IpcContract;
type NoArgsCommand = {
  [Command in IpcCommand]: IpcContract[Command]["args"] extends undefined ? Command : never;
}[IpcCommand];
type ArgsCommand = Exclude<IpcCommand, NoArgsCommand>;

export function invoke<Command extends NoArgsCommand>(
  command: Command,
): Promise<IpcContract[Command]["result"]>;
export function invoke<Command extends ArgsCommand>(
  command: Command,
  args: IpcContract[Command]["args"],
): Promise<IpcContract[Command]["result"]>;
export function invoke(command: IpcCommand, args?: unknown): Promise<unknown> {
  return tauriInvoke(command, args as Record<string, unknown> | undefined);
}
