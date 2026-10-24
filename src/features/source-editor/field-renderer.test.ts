import { describe, expect, it, vi } from "vitest";

import type { FieldDef } from "../../source-types/forms";
import { buildFieldActionRow, credentialManagedKeysOf, renderFieldsInto } from "./field-renderer";

function rowOptions(pickers = {}) {
  return {
    idPrefix: "fd-",
    sourceType: "excel_range",
    label: document.createElement("label"),
    wireCredentialSelector: vi.fn(),
    pickers,
  };
}

describe("field action row", () => {
  it("returns null when the field has no adjacent actions", () => {
    const field: FieldDef = { key: "name", label: "名前", kind: "text" };
    expect(buildFieldActionRow(field, document.createElement("input"), rowOptions())).toBeNull();
  });

  it("places the browse button before picker buttons and calls the matching picker", () => {
    const openCellPicker = vi.fn();
    const field: FieldDef = { key: "path", label: "セル", kind: "text", browse: "file", browseCell: "bottom" };
    const input = document.createElement("input");
    const row = buildFieldActionRow(field, input, rowOptions({ openCellPicker }))!;

    const buttons = [...row.querySelectorAll("button")];
    expect(buttons.map((b) => b.textContent)).toEqual(["参照…", "表から選択…"]);
    buttons[1].click();
    expect(openCellPicker).toHaveBeenCalledWith(input, "bottom", "cell");
  });

  it("omits picker buttons whose callback the host does not provide", () => {
    const field: FieldDef = { key: "sheet", label: "シート", kind: "text", browseSheet: true };
    expect(buildFieldActionRow(field, document.createElement("input"), rowOptions())).toBeNull();
  });

  it("hides credential bundle fields and keeps help links on the credential widget", () => {
    const fields: FieldDef[] = [
      { key: "client_secret", label: "シークレット", kind: "password", credentialScope: "graph", credentialBundleFields: ["tenant_id"], helpUrl: "https://example.test" },
      { key: "tenant_id", label: "テナント", kind: "text", placeholder: "xxxx" },
    ];
    expect([...credentialManagedKeysOf(fields)]).toEqual(["client_secret", "tenant_id"]);

    const host = document.createElement("div");
    const wireCredentialSelector = vi.fn();
    renderFieldsInto(host, "pf-", fields, {
      wireCredentialSelector,
      openAirtableBasePicker: vi.fn(),
      openAirtableTablePicker: vi.fn(),
      openAirtableViewPicker: vi.fn(),
    }, "graph_entra_users");

    const tenant = host.querySelector<HTMLInputElement>("#pf-tenant_id")!;
    expect(tenant.hidden).toBe(true);
    expect(tenant.placeholder).toBe("");
    expect(wireCredentialSelector).toHaveBeenCalledOnce();
    expect(host.querySelectorAll("button")).toHaveLength(0);
  });
});
