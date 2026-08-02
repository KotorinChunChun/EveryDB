import { mountCredentialManager, type CredentialManagerController, type CredentialManagerDeps } from "./manager";
import { mountCredentialSelector, type CredentialSelectorController, type CredentialSelectorDeps } from "./selector";
import { createWinAuth, type WinAuthController } from "./winauth";

export type { CredentialManagerController, CredentialManagerDeps } from "./manager";
export type { CredentialSelectorController, CredentialSelectorDeps } from "./selector";
export type { WinAuthController } from "./winauth";
export { createWinAuth } from "./winauth";
export { mountCredentialManager } from "./manager";
export { mountCredentialSelector } from "./selector";

export interface CredentialsDeps {
  getSourceEditor: CredentialSelectorDeps["getSourceEditor"];
  openGuide: CredentialManagerDeps["openGuide"];
}

export interface CredentialsController extends WinAuthController, CredentialManagerController, CredentialSelectorController {}

/** Windows Hello + トークン選択ピッカー + トークン管理画面を一括でマウントする。 */
export function mountCredentials(deps: CredentialsDeps): CredentialsController {
  const winAuth = createWinAuth();
  const manager = mountCredentialManager({ winAuth, openGuide: deps.openGuide });
  const selector = mountCredentialSelector({ getSourceEditor: deps.getSourceEditor, manager });

  return {
    ...winAuth,
    ...manager,
    ...selector,
  };
}
