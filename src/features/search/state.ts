/** 検索上限の入力値を、バックエンドへ渡せる安全な件数に正規化する。 */
export function normalizeRowLimit(value: string, fallback = 10_000): number {
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

/** 非同期検索応答が、すでに描画済みの結果より新しいか判定する。 */
export function shouldApplySearchResponse(responseSeq: number, lastAppliedSeq: number): boolean {
  return responseSeq > lastAppliedSeq;
}

/** プリセットに含まれるうち、現在も存在するソースだけを選択対象にする。 */
export function presetSourceIds(sourceIds: Iterable<number>, validSourceIds: Iterable<number>): Set<number> {
  const valid = new Set(validSourceIds);
  return new Set([...sourceIds].filter((id) => valid.has(id)));
}