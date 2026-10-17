/** ハイフン表記の違いを無視して、同じNotionデータベースIDか判定できる形へ揃える。 */
export function normalizeNotionDatabaseId(databaseId: string): string {
  return databaseId.trim().replaceAll("-", "").toLowerCase();
}

/** Notionの接続先表示名はAPIのデータベース名が正本であり、利用者編集を許可しない。 */
export function isApiOwnedSourceName(type_: string): boolean {
  return type_ === "notion";
}

/** 現在のIDに対応するNotion名が取得済みで、保存してよい状態かを返す。 */
export function isNotionNameResolved(
  databaseId: string,
  resolvedDatabaseId: string,
  databaseName: string,
): boolean {
  const current = normalizeNotionDatabaseId(databaseId);
  return current.length === 32
    && current === normalizeNotionDatabaseId(resolvedDatabaseId)
    && databaseName.trim().length > 0;
}

/** 名称取得失敗時に、保存値を維持していることまで利用者へ伝える。 */
export function notionNameFailureMessage(error: unknown, hasSavedName: boolean): string {
  const detail = String(error);
  return hasSavedName
    ? `最新名の取得に失敗しました。最後に取得できた名前を維持しています: ${detail}`
    : `データベース名の取得に失敗しました。接続設定を確認して再試行してください: ${detail}`;
}
