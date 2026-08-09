import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import html from "../../../index.html?raw";
import { mountCredentialManager } from "./manager";

const invokeMock = vi.mocked(tauriInvoke);

describe("personal Microsoft credential manager", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (command) => {
      if (command === "list_credentials") return [];
      if (command === "microsoft_account_oauth_authorize") {
        return { refresh_token: "personal-refresh" };
      }
      return undefined;
    });
    Object.defineProperty(HTMLDialogElement.prototype, "showModal", {
      configurable: true,
      value(this: HTMLDialogElement) {
        this.setAttribute("open", "");
      },
    });
    document.documentElement.innerHTML = html;
  });

  it("authorizes with the bundled client ID without showing or sending an input value", async () => {
    const manager = mountCredentialManager({
      winAuth: {
        refreshWinAuthStatus: vi.fn(async () => {}),
        ensureUnlocked: vi.fn(async () => true),
      },
      openGuide: vi.fn(),
      openGuideDoc: vi.fn(),
    });

    await manager.openCredentialManager("microsoft_account");

    expect(document.getElementById("cmf-client_id")).toBeNull();
    expect(document.getElementById("cm-form-fields")?.textContent).not.toContain("クライアントID");

    const authorize = [...document.querySelectorAll<HTMLButtonElement>("#cm-form-fields button")]
      .find((button) => button.textContent?.includes("個人 Microsoft アカウントで認可"));
    expect(authorize).toBeDefined();
    authorize!.click();

    await vi.waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("microsoft_account_oauth_authorize", undefined);
      expect((document.getElementById("cmf-refresh_token") as HTMLInputElement).value)
        .toBe("personal-refresh");
    });
  });

  it("requires one-time reauthorization for a profile created with a different client ID", async () => {
    invokeMock.mockImplementation(async (command) => {
      if (command === "list_credentials") {
        return [{
          id: 7,
          scope: "microsoft_account",
          label: "旧個人OneDrive",
          created_at: "2026-08-09",
          used_by: [],
        }];
      }
      if (command === "credential_public_fields") {
        return {
          label: "旧個人OneDrive",
          fields: { _requires_reauthorization: true },
        };
      }
      return undefined;
    });
    const manager = mountCredentialManager({
      winAuth: {
        refreshWinAuthStatus: vi.fn(async () => {}),
        ensureUnlocked: vi.fn(async () => true),
      },
      openGuide: vi.fn(),
      openGuideDoc: vi.fn(),
    });

    await manager.openCredentialManager("microsoft_account");
    (document.querySelector('[data-credential-id="7"]') as HTMLElement).click();

    await vi.waitFor(() => {
      expect((document.getElementById("cmf-refresh_token") as HTMLInputElement).value).toBe("");
      expect(document.getElementById("cm-error")?.textContent).toContain("再認可が必要です");
      expect(document.getElementById("cm-form-fields")?.textContent).toContain("未認可");
    });
  });

  it("shows approved Microsoft permissions in the saved token list", async () => {
    invokeMock.mockImplementation(async (command) => {
      if (command === "list_credentials") {
        return [{
          id: 11,
          scope: "graph",
          label: "Entra 読み取り",
          created_at: "2026-08-09",
          used_by: [],
        }];
      }
      if (command === "credential_permissions") {
        return {
          auth_method: "app_only",
          claim: "roles",
          permissions: ["Group.Read.All", "User.Read.All"],
        };
      }
      return undefined;
    });
    const manager = mountCredentialManager({
      winAuth: {
        refreshWinAuthStatus: vi.fn(async () => {}),
        ensureUnlocked: vi.fn(async () => true),
      },
      openGuide: vi.fn(),
      openGuideDoc: vi.fn(),
    });

    await manager.openCredentialManager("graph");

    await vi.waitFor(() => {
      expect(document.querySelector('[data-credential-id="11"]')?.textContent)
        .toContain("承認済み権限（アプリケーション / roles）: Group.Read.All, User.Read.All");
    });
  });
});
