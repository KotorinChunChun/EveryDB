import { invoke } from "../../app/ipc";
import { appState, setWinAuthStatus } from "../../app/state";
import type { WinAuthStatus } from "../../shared/types";

export interface WinAuthController {
  refreshWinAuthStatus: () => Promise<void>;
  ensureUnlocked: () => Promise<boolean>;
}

/** Windows Hello 本人確認（シークレット閲覧・使用の前に呼ぶ）。 */
export function createWinAuth(): WinAuthController {
  async function refreshWinAuthStatus() {
    setWinAuthStatus(await invoke<WinAuthStatus>("winauth_status"));
  }

  /** シークレットを表示/使用する操作の前に本人確認を保証する。
   * 直近 10 分以内に確認済みなら省略する。解錠済みなら即 true。 */
  async function ensureUnlocked(): Promise<boolean> {
    await refreshWinAuthStatus();
    if (appState.winAuthStatus.unlocked) return true;
    try {
      const ok = await invoke<boolean>("winauth_verify");
      await refreshWinAuthStatus();
      return ok;
    } catch (e) {
      alert("本人確認に失敗しました: " + String(e));
      return false;
    }
  }

  return { refreshWinAuthStatus, ensureUnlocked };
}
