import { describe, expect, it } from "vitest";
import { buildExportText, encodeExportText } from "./serialization";

type Row = string[];
const columns = ["名前", "メモ"];
const rows: Row[] = [["Alice", "a,b"], ["Bob", "line1\nline2"]];
const value = (row: Row, columnIndex: number) => row[columnIndex];

describe("export serialization", () => {
  it("escapes CSV and flattens TSV cell line breaks", () => {
    expect(buildExportText("csv", columns, rows, value)).toBe('名前,メモ\r\nAlice,"a,b"\r\nBob,"line1\nline2"');
    expect(buildExportText("tsv", columns, rows, value)).toBe("名前\tメモ\r\nAlice\ta,b\r\nBob\tline1 line2");
  });

  it("keeps column names in JSON and encodes BOM and Shift_JIS through the injected boundary", async () => {
    expect(buildExportText("json", columns, rows, value)).toContain('"名前": "Alice"');
    await expect(encodeExportText("A", "utf8bom", async () => new Uint8Array())).resolves.toEqual(new Uint8Array([0xef, 0xbb, 0xbf, 0x41]));
    await expect(encodeExportText("日本語", "sjis", async () => new Uint8Array([1, 2]))).resolves.toEqual(new Uint8Array([1, 2]));
  });
});