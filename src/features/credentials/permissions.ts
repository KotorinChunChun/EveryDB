import { invoke, type CredentialPermissionInspection } from "../../app/ipc";

export function supportsCredentialPermissionInspection(scope: string): boolean {
  return scope === "graph" || scope === "microsoft_account";
}

export function credentialPermissionLabel(result: CredentialPermissionInspection): string {
  const auth = result.auth_method === "app_only" ? "アプリケーション" : "委任";
  const permissions = result.permissions.length > 0 ? result.permissions.join(", ") : "なし";
  return `承認済み権限（${auth} / ${result.claim}）: ${permissions}`;
}

/** 権限確認は選択可否から独立させる。通信失敗時も行を無効化しない。 */
export async function populateCredentialPermissionStatus(id: number, status: HTMLElement) {
  status.dataset.state = "loading";
  status.textContent = "承認済み権限を確認中…";
  try {
    const result = await invoke("credential_permissions", { id });
    status.dataset.state = "success";
    status.textContent = credentialPermissionLabel(result);
  } catch (error) {
    status.dataset.state = "error";
    status.textContent = `権限確認失敗: ${String(error)}`;
  }
}
