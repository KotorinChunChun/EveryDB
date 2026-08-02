export type ExportFormat = "tsv" | "csv" | "json";
export type ExportEncoding = "utf8bom" | "utf8" | "sjis";

export const EXPORT_FORMAT_INFO: Record<ExportFormat, { ext: string; filterName: string; extra: string[] }> = {
  tsv: { ext: "tsv", filterName: "TSV", extra: ["txt"] },
  csv: { ext: "csv", filterName: "CSV", extra: [] },
  json: { ext: "json", filterName: "JSON", extra: [] },
};

function tsvEscape(value: string): string {
  return value.replace(/[\t\r\n]+/g, " ");
}

function csvEscape(value: string): string {
  return /[,"\r\n]/.test(value) ? `"${value.replace(/"/g, '""')}"` : value;
}

/** 現在のグリッドをTSV/CSV/JSONの文字列へ変換する。 */
export function buildExportText<Row>(
  format: ExportFormat,
  columns: readonly string[],
  rows: readonly Row[],
  cellText: (row: Row, columnIndex: number, rowIndex: number) => string,
): string {
  if (format === "json") {
    const data = rows.map((row, rowIndex) =>
      Object.fromEntries(columns.map((column, columnIndex) => [column, cellText(row, columnIndex, rowIndex)])),
    );
    return JSON.stringify(data, null, 2);
  }
  const escape = format === "csv" ? csvEscape : tsvEscape;
  const separator = format === "csv" ? "," : "\t";
  const header = columns.map(escape).join(separator);
  const lines = rows.map((row, rowIndex) => columns.map((_, columnIndex) => escape(cellText(row, columnIndex, rowIndex))).join(separator));
  return [header, ...lines].join("\r\n");
}

/** 指定文字コードのバイト列を作る。Shift_JISの実装だけはRust側へ注入する。 */
export async function encodeExportText(
  text: string,
  encoding: ExportEncoding,
  encodeShiftJis: (text: string) => Promise<Uint8Array>,
): Promise<Uint8Array> {
  if (encoding === "sjis") return encodeShiftJis(text);
  const body = new TextEncoder().encode(text);
  if (encoding === "utf8bom") {
    const withBom = new Uint8Array(body.length + 3);
    withBom.set([0xef, 0xbb, 0xbf]);
    withBom.set(body, 3);
    return withBom;
  }
  return body;
}