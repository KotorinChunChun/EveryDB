import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { FieldDef } from "../../source-types/forms";
import {
  graphCredentialCompatibility,
  graphUsesSignedInUser,
  mountCredentialSelector,
  updateGraphSelfSubjectUi,
} from "./selector";

beforeEach(() => {
  vi.mocked(tauriInvoke).mockReset();
});

describe("Graph credential compatibility in the picker", () => {
  it("keeps legacy app-only profiles compatible with every Graph source", () => {
    for (const sourceType of ["graph_teams", "graph_sharepoint_list", "graph_calendar", "graph_onedrive"]) {
      expect(graphCredentialCompatibility(sourceType, {}).compatible).toBe(true);
    }
  });

  it("keeps incompatible profiles visible but disabled in the picker", async () => {
    document.body.innerHTML = `
      <dialog id="credential-select-picker">
        <h3 id="credential-select-picker-title"></h3>
        <div id="credential-select-picker-list"></div>
        <button id="credential-select-picker-manage"></button>
        <button id="credential-select-picker-close"></button>
      </dialog>`;
    const dialog = document.getElementById("credential-select-picker") as HTMLDialogElement;
    Object.defineProperty(dialog, "showModal", { value: vi.fn() });
    Object.defineProperty(dialog, "close", { value: vi.fn() });
    vi.mocked(tauriInvoke).mockImplementation(async (command, args) => {
      if (command === "list_credentials") {
        return [
          { id: 1, scope: "graph", label: "アプリ認証", created_at: "today", used_by: [] },
          { id: 2, scope: "graph", label: "SharePoint委任", created_at: "today", used_by: [] },
        ];
      }
      if (command === "credential_public_fields") {
        const id = (args as { id: number }).id;
        return id === 1
          ? { label: "アプリ認証", fields: { auth_method: "app_only" } }
          : { label: "SharePoint委任", fields: { auth_method: "delegated", capabilities: ["sharepoint"] } };
      }
      throw new Error(`unexpected command: ${command}`);
    });
    const controller = mountCredentialSelector({
      getSourceEditor: () => ({
        maybeFetchAndRenderFields: vi.fn(),
        autoFillDefaultName: vi.fn(),
      }) as never,
      manager: {
        openCredentialManager: vi.fn(),
        copyCredentialSecretToClipboard: vi.fn(),
        copyCredentialBundleAsJson: vi.fn(),
      },
    });
    const input = document.createElement("input");
    const row = document.createElement("div");
    const label = document.createElement("label");
    const field: FieldDef = { key: "client_secret", label: "secret", kind: "password", credentialScope: "graph" };
    controller.wireCredentialSelector(input, field, "fd-", row, label, "graph_teams");

    (row.querySelector("button") as HTMLButtonElement).click();
    await vi.waitFor(() => {
      expect(document.querySelectorAll("#credential-select-picker-list .credential-picker-item")).toHaveLength(2);
    });
    const items = [...document.querySelectorAll<HTMLButtonElement>(".credential-picker-item")];
    expect(items[0].disabled).toBe(false);
    expect(items[1].disabled).toBe(true);
    expect(items[1].textContent).toContain("委任 OAuth に対応していません");
  });

  it("treats legacy delegated profiles as SharePoint capability", () => {
    const profile = { auth_method: "delegated" };
    expect(graphCredentialCompatibility("graph_sharepoint_library", profile)).toMatchObject({
      compatible: true,
      capabilities: ["sharepoint"],
    });
    expect(graphCredentialCompatibility("graph_calendar", profile)).toMatchObject({
      compatible: false,
      reason: expect.stringContaining("calendar_self"),
    });
  });

  it("disables delegated profiles for app-only sources with a visible reason", () => {
    const result = graphCredentialCompatibility("graph_teams", {
      auth_method: "delegated",
      capabilities: ["sharepoint"],
    });
    expect(result.compatible).toBe(false);
    expect(result.reason).toContain("委任 OAuth に対応していません");
  });

  it("accepts delegated profiles only when the required capability is present", () => {
    expect(graphCredentialCompatibility("graph_calendar", {
      auth_method: "delegated",
      capabilities: ["calendar_self"],
    }).compatible).toBe(true);
    expect(graphCredentialCompatibility("graph_onedrive", {
      auth_method: "delegated",
      capabilities: ["calendar_self"],
    })).toMatchObject({
      compatible: false,
      reason: expect.stringContaining("onedrive_self"),
    });
  });

  it("disables UPN and explains signed-in-user scope for delegated Calendar", () => {
    document.body.innerHTML = `<label>対象ユーザー<input id="fd-user"></label>`;
    expect(graphUsesSignedInUser("graph_calendar", { auth_method: "delegated" })).toBe(true);
    updateGraphSelfSubjectUi("graph_calendar", "fd-", { auth_method: "delegated" });
    const input = document.getElementById("fd-user") as HTMLInputElement;
    expect(input.disabled).toBe(true);
    expect(input.closest("label")?.textContent).toContain("サインインした本人");

    updateGraphSelfSubjectUi("graph_calendar", "fd-", { auth_method: "app_only" });
    expect(input.disabled).toBe(false);
  });
});
