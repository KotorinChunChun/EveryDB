import { SECRET_PLACEHOLDER } from "./constants";

/** 接続設定の共有キーが実際に変わったか（秘密のプレースホルダー・同一 cred 参照は未変更扱い）。 */
export function sharedKeyValueChanged(
  key: string,
  newRaw: string,
  oldRaw: string,
  normalizePath: (value: string | undefined | null) => string,
): boolean {
  if (newRaw === SECRET_PLACEHOLDER) return false;
  const norm = (v: string) => (key === "path" ? normalizePath(v) : v.trim());
  return norm(newRaw) !== norm(oldRaw);
}

/** 新しい共有値を既存 config に適用すると変更が生じるか。 */
export function sharedConfigWouldChange(
  existingConfig: Record<string, unknown>,
  sharedValues: Record<string, unknown>,
  keys: string[],
  normalizePath: (value: string | undefined | null) => string,
): boolean {
  for (const k of keys) {
    if (!(k in sharedValues)) continue;
    const newRaw = String(sharedValues[k] ?? "");
    const oldRaw = String(existingConfig[k] ?? "");
    if (sharedKeyValueChanged(k, newRaw, oldRaw, normalizePath)) return true;
  }
  return false;
}
