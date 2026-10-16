import { describe, expect, it } from "vitest";
import { TYPE_DEFS } from "./forms";
import { SOURCE_TYPE_METADATA } from "./registry";

describe("source type forms", () => {
  it("defines exactly one form for every frontend manifest entry", () => {
    const formIds = TYPE_DEFS.map((definition) => definition.value);
    const manifestIds = SOURCE_TYPE_METADATA.map((metadata) => metadata.id);

    expect(new Set(formIds).size).toBe(formIds.length);
    expect([...formIds].sort()).toEqual([...manifestIds].sort());
    expect(TYPE_DEFS.every((definition) => definition.label.trim().length > 0)).toBe(true);
    expect(
      TYPE_DEFS.every((definition) =>
        definition.label === SOURCE_TYPE_METADATA.find((metadata) => metadata.id === definition.value)!.label,
      ),
    ).toBe(true);
    expect(
      TYPE_DEFS.every((definition) =>
        (definition.browse ?? null)
          === SOURCE_TYPE_METADATA.find((metadata) => metadata.id === definition.value)!.browse,
      ),
    ).toBe(true);
  });

  it("keeps credential fields aligned with manifest credential scopes", () => {
    for (const metadata of SOURCE_TYPE_METADATA) {
      const definition = TYPE_DEFS.find((item) => item.value === metadata.id)!;
      const fieldScopes = new Set(
        definition.fields.flatMap((field) => field.credentialScope ? [field.credentialScope] : []),
      );
      const passwordFields = definition.fields.filter((field) => field.kind === "password");

      if (metadata.credentialScope) {
        expect(fieldScopes).toEqual(new Set([metadata.credentialScope]));
        expect(passwordFields.length).toBeGreaterThan(0);
      } else {
        expect(fieldScopes.size).toBe(0);
      }
      expect(passwordFields.every((field) => field.credentialScope !== undefined)).toBe(true);
    }
  });

  it("gives personal OneDrive only a personal Microsoft credential field", () => {
    const definition = TYPE_DEFS.find((item) => item.value === "microsoft_account_onedrive")!;
    expect(definition.fields.map((field) => field.key)).toEqual(["access_token"]);
    expect(definition.fields[0].credentialScope).toBe("microsoft_account");
    expect(definition.fields.some((field) => field.key === "tenant_id" || field.key === "user")).toBe(false);
  });

  it("labels browser profile paths consistently", () => {
    for (const type of ["browser_history", "browser_bookmarks", "browser_downloads"]) {
      const pathField = TYPE_DEFS.find((item) => item.value === type)!.fields
        .find((field) => field.key === "path");
      expect(pathField?.label).toBe("プロファイル");
      expect(pathField?.browserProfile).toBeDefined();
    }
  });

  it("keeps the Notion database name API-owned and required", () => {
    const definition = TYPE_DEFS.find((item) => item.value === "notion")!;
    const nameField = definition.fields.find((field) => field.key === "database_name")!;
    expect(nameField.readOnly).toBe(true);
    expect(nameField.optional).not.toBe(true);
  });
});
