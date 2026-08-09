import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  credentialPermissionLabel,
  populateCredentialPermissionStatus,
  supportsCredentialPermissionInspection,
} from "./permissions";

describe("credential permission status", () => {
  beforeEach(() => {
    vi.mocked(tauriInvoke).mockReset();
  });

  it("supports only Microsoft credential scopes", () => {
    expect(supportsCredentialPermissionInspection("graph")).toBe(true);
    expect(supportsCredentialPermissionInspection("microsoft_account")).toBe(true);
    expect(supportsCredentialPermissionInspection("airtable")).toBe(false);
    expect(supportsCredentialPermissionInspection("notion")).toBe(false);
    expect(supportsCredentialPermissionInspection("box")).toBe(false);
  });

  it("formats the effective claim and permissions", () => {
    expect(credentialPermissionLabel({
      auth_method: "app_only",
      claim: "roles",
      permissions: ["Group.Read.All", "User.Read.All"],
    })).toBe("承認済み権限（アプリケーションの許可）: Group.Read.All, User.Read.All");
  });

  it("shows a communication failure in place without disabling its parent button", async () => {
    vi.mocked(tauriInvoke).mockRejectedValue(new Error("network offline"));
    const button = document.createElement("button");
    const status = document.createElement("small");
    button.appendChild(status);

    await populateCredentialPermissionStatus(9, status);

    expect(status.dataset.state).toBe("error");
    expect(status.textContent).toContain("権限確認失敗");
    expect(button.disabled).toBe(false);
  });
});
