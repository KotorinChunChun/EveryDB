export interface AirtableUrlInfo {
  baseId?: string;
  tableId?: string;
  viewId?: string;
}

/** セル内容から安全に opener へ渡せるURLだけを返す。 */
export function getCellUrl(text: string): string | null {
  const trimmed = text.trim();
  if (!trimmed) return null;
  if (/^(https?|file|notion|ftp|mailto|teams|obsidian):/i.test(trimmed)) {
    return trimmed;
  }
  return /^www\./i.test(trimmed) ? `https://${trimmed}` : null;
}

export function parseAirtableUrl(urlText: string): AirtableUrlInfo | null {
  if (!urlText || !urlText.includes("airtable.com")) return null;
  const baseMatch = urlText.match(/\b(app[a-zA-Z0-9]{11,17})\b/);
  const tableMatch = urlText.match(/\b(tbl[a-zA-Z0-9]{11,17})\b/);
  const viewMatch = urlText.match(/\b(viw[a-zA-Z0-9]{11,17})\b/);
  if (!baseMatch && !tableMatch && !viewMatch) return null;
  return {
    baseId: baseMatch?.[1],
    tableId: tableMatch?.[1],
    viewId: viewMatch?.[1],
  };
}

export function parseGoogleSheetsUrl(urlText: string): { spreadsheetId?: string } | null {
  if (!urlText || !urlText.includes("docs.google.com/spreadsheets")) return null;
  const match = urlText.match(/\/spreadsheets\/d\/([a-zA-Z0-9-_]+)/);
  return match ? { spreadsheetId: match[1] } : null;
}

export function parseNotionUrl(urlText: string): { databaseId?: string } | null {
  if (!urlText || (!urlText.includes("notion.so") && !urlText.includes("notion.site"))) return null;
  const match = urlText.match(/([0-9a-fA-F]{32}|[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})/);
  return match ? { databaseId: match[1].replace(/-/g, "").toLowerCase() } : null;
}

export function parseSharePointUrl(urlText: string): { siteUrl?: string; listName?: string; libraryName?: string } | null {
  if (!urlText || !urlText.includes("sharepoint.com")) return null;
  const siteMatch = urlText.match(/(https:\/\/[a-zA-Z0-9.-]+\.sharepoint\.com\/(?:sites|teams)\/[^/?#]+)/i);
  const listMatch = urlText.match(/\/Lists\/([^/?#]+)/i);
  const libraryMatch = urlText.match(/\/sites\/[^/]+\/([^/?#]+)\/(?:Forms|AllItems)/i);
  const decode = (value: string | undefined) => {
    if (!value) return undefined;
    try {
      return decodeURIComponent(value);
    } catch {
      return value;
    }
  };
  const libraryName = libraryMatch?.[1] === "Lists" ? undefined : decode(libraryMatch?.[1]);
  const result = { siteUrl: siteMatch?.[1], listName: decode(listMatch?.[1]), libraryName };
  return result.siteUrl || result.listName || result.libraryName ? result : null;
}
