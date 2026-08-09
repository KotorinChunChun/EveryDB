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
        return {
          refresh_token: "personal-refresh",
          authorized_account_id: "account-guid",
          authorized_account_name: "user@example.com",
        };
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
      expect(document.getElementById("cm-form-fields")?.textContent)
        .toContain("認可済み: user@example.com（アカウントID: account-guid）");
    });

    const label = document.getElementById("cm-label") as HTMLInputElement;
    label.value = "個人OneDrive";
    label.dispatchEvent(new Event("input"));
    (document.getElementById("cm-form-save") as HTMLButtonElement).click();
    await vi.waitFor(() => {
      const call = invokeMock.mock.calls.find(([command]) => command === "add_credential");
      expect(call).toBeDefined();
      const fields = JSON.parse((call?.[1] as { fieldsJson: string }).fieldsJson);
      expect(fields).toMatchObject({
        refresh_token: "personal-refresh",
        authorized_account_id: "account-guid",
        authorized_account_name: "user@example.com",
      });
    });
  });

  it("marks an existing authorized profile without identity as not recorded", async () => {
    invokeMock.mockImplementation(async (command) => {
      if (command === "list_credentials") {
        return [{
          id: 8,
          scope: "microsoft_account",
          label: "既存個人OneDrive",
          created_at: "2026-08-09",
          used_by: [],
        }];
      }
      if (command === "credential_public_fields") {
        return { label: "既存個人OneDrive", fields: {} };
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
    (document.querySelector('[data-credential-id="8"]') as HTMLElement).click();

    await vi.waitFor(() => {
      expect(document.getElementById("cm-form-fields")?.textContent)
        .toContain("認可済み（認可アカウント未記録）");
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
          used_by: [{ source_id: 3, source_name: "EntraID ユーザー" }],
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
        .toContain("承認済み権限（アプリケーションの許可）: Group.Read.All, User.Read.All");
    });
    const row = document.querySelector('[data-credential-id="11"]') as HTMLElement;
    expect(row.querySelector(".cm-profile-usage")?.textContent).toContain("EntraID ユーザー");
    expect(row.nextElementSibling?.classList.contains("cm-profile-usage")).not.toBe(true);
  });
});
