import type { Row } from "../../shared/types";
import { gridCellText } from "./data";

export function formatGridCellText(
  row: Row,
  columnName: string,
  rowIndex?: number,
): string {
  return gridCellText(row, columnName, rowIndex);
}

export function rowCellTexts(
  row: Row,
  columns: readonly string[],
  rowIndex?: number,
): string[] {
  return columns.map((column, index) => formatGridCellText(row, column, rowIndex ?? index));
}

export function rowAsTsv(row: Row, columns: readonly string[], rowIndex?: number): string {
  return rowCellTexts(row, columns, rowIndex).join("\t");
}

export function rowAsTsvWithHeader(row: Row, columns: readonly string[], rowIndex?: number): string {
  const pairs = columns
    .map((column, index) => [column, formatGridCellText(row, column, rowIndex ?? index)] as const)
    .filter(([, value]) => value !== "");
  const header = pairs.map(([column]) => column).join("\t");
  const values = pairs.map(([, value]) => value).join("\t");
  return `${header}\n${values}`;
}

export function rowAsJson(row: Row, columns: readonly string[], rowIndex?: number): string {
  const obj: Record<string, string> = {};
  columns.forEach((column, index) => {
    obj[column] = formatGridCellText(row, column, rowIndex ?? index);
  });
  return JSON.stringify(obj, null, 2);
}
