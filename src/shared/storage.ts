export const STORAGE_KEY_ROW_LIMIT = "everydb_row_limit";
export const STORAGE_KEY_SELECTED_SOURCES = "everydb_selected_source_ids";
export const STORAGE_KEY_COLLAPSED_PARENTS = "everydb_collapsed_parents";

export function saveString(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    // ストレージが利用できない環境では画面操作を妨げない。
  }
}

export function loadString(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

export function saveJson(key: string, value: unknown): void {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // ストレージが利用できない環境では画面操作を妨げない。
  }
}

function loadJson(key: string): unknown | null {
  try {
    const raw = localStorage.getItem(key);
    return raw ? JSON.parse(raw) : null;
  } catch {
    return null;
  }
}

export function loadNumberArray(key: string): number[] | null {
  const value = loadJson(key);
  return Array.isArray(value) && value.every((item) => typeof item === "number") ? value : null;
}

export function loadStringArray(key: string): string[] | null {
  const value = loadJson(key);
  return Array.isArray(value) && value.every((item) => typeof item === "string") ? value : null;
}
