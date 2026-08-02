export function colLetterOf(columnIndex: number): string {
  let result = "";
  let number = columnIndex + 1;
  while (number > 0) {
    const remainder = (number - 1) % 26;
    result = String.fromCharCode(65 + remainder) + result;
    number = Math.floor((number - 1) / 26);
  }
  return result;
}

export function colLetterToIndex(column: string): number {
  const value = column.trim().toUpperCase();
  if (!/^[A-Z]+$/.test(value)) return -1;
  let result = 0;
  for (let index = 0; index < value.length; index += 1) {
    result = result * 26 + (value.charCodeAt(index) - 64);
  }
  return result - 1;
}

export function parseColumnSelection(text: string): Set<number> {
  const selected = new Set<number>();
  for (const part of text.split(/[,;\s]+/)) {
    const value = part.trim().toUpperCase();
    if (!value) continue;
    if (value.includes(":")) {
      const [start, end] = value.split(":");
      const startIndex = colLetterToIndex(start);
      const endIndex = colLetterToIndex(end);
      if (startIndex < 0 || endIndex < 0) continue;
      for (let index = Math.min(startIndex, endIndex); index <= Math.max(startIndex, endIndex); index += 1) {
        selected.add(index);
      }
    } else {
      const index = colLetterToIndex(value);
      if (index >= 0) selected.add(index);
    }
  }
  return selected;
}

export function formatSelectedColumns(selectedColumns: Set<number>): string {
  const columns = [...selectedColumns].sort((left, right) => left - right);
  if (columns.length === 0) return "";
  const ranges: string[] = [];
  let start = columns[0];
  let end = columns[0];
  for (const column of columns.slice(1)) {
    if (column === end + 1) {
      end = column;
      continue;
    }
    ranges.push(start === end ? colLetterOf(start) : `${colLetterOf(start)}:${colLetterOf(end)}`);
    start = column;
    end = column;
  }
  ranges.push(start === end ? colLetterOf(start) : `${colLetterOf(start)}:${colLetterOf(end)}`);
  return ranges.join(", ");
}
