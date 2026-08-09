import { describe, expect, it } from "vitest";
import {
  SOURCE_TYPE_MANIFEST,
  SOURCE_TYPE_METADATA,
  browseCapabilityForType,
  graphCredentialRequirementsFor,
  guideNameForType,
  openCapabilityForType,
  parentKindForType,
  representativeTypeForCredentialScope,
} from "./registry";

describe("source type metadata", () => {
  it("has unique, complete metadata for every registered type", () => {
    const ids = SOURCE_TYPE_METADATA.map((item) => item.id);
    const parentKinds = new Set(["path", "sharepoint", "notion", "airtable", "graph365", "postgres", "mysql", "box", "none"]);
    const categories = new Set(["local", "microsoft", "cloud", "database", "public-data", "web"]);
    const browseCapabilities = new Set([null, "notion", "sqlite", "accdb", "excel_table", "sharepoint", "airtable"]);
    const openCapabilities = new Set(["none", "table", "parent", "table-and-parent"]);

    expect(new Set(ids).size).toBe(ids.length);
    expect(SOURCE_TYPE_METADATA.every((item) => item.label.trim().length > 0 && item.guide.length > 0)).toBe(true);
    expect(SOURCE_TYPE_METADATA.every((item) => parentKinds.has(item.parentKind) && categories.has(item.category))).toBe(true);
    expect(SOURCE_TYPE_METADATA.every((item) => browseCapabilities.has(item.browse))).toBe(true);
    expect(SOURCE_TYPE_METADATA.every((item) => openCapabilities.has(item.open))).toBe(true);
  });

  it("declares Graph auth methods and delegated capabilities for every Graph source", () => {
    const graphEntries = SOURCE_TYPE_METADATA.filter((item) => item.credentialScope === "graph");
    expect(graphEntries.length).toBeGreaterThan(0);
    expect(graphEntries.every((item) => item.credentialAuthMethods?.includes("app_only"))).toBe(true);
    expect(graphEntries.every((item) => Array.isArray(item.requiredCredentialCapabilities))).toBe(true);

    expect(graphCredentialRequirementsFor("graph_teams")).toEqual({
      authMethods: ["app_only"],
      capabilities: [],
    });
    expect(graphCredentialRequirementsFor("graph_sharepoint_list")).toEqual({
      authMethods: ["app_only", "delegated"],
      capabilities: ["sharepoint"],
    });
    expect(graphCredentialRequirementsFor("graph_calendar")).toEqual({
      authMethods: ["app_only", "delegated"],
      capabilities: ["calendar_self"],
    });
    expect(graphCredentialRequirementsFor("graph_onedrive")).toEqual({
      authMethods: ["app_only", "delegated"],
      capabilities: ["onedrive_self"],
    });
  });

  it("keeps the legacy Excel cell type out of new source selection", () => {
    expect(SOURCE_TYPE_MANIFEST.find((item) => item.id === "excel_cell")).toMatchObject({ frontend: false });
    expect(SOURCE_TYPE_METADATA.some((item) => item.id === "excel_cell")).toBe(false);
  });

  it("keeps grouped and credential-bearing types on their expected contracts", () => {
    expect(parentKindForType("excel_table")).toBe("path");
    expect(parentKindForType("graph_sharepoint_list")).toBe("sharepoint");
    expect(browseCapabilityForType("graph_sharepoint_list")).toBe("sharepoint");
    expect(openCapabilityForType("graph_sharepoint_list")).toBe("table-and-parent");
    expect(openCapabilityForType("postgres")).toBe("none");
    expect(guideNameForType("box_files")).toBe("box");
    expect(representativeTypeForCredentialScope("airtable")).toBe("airtable");
    expect(parentKindForType("unknown")).toBe("none");
    expect(browseCapabilityForType("unknown")).toBeNull();
    expect(openCapabilityForType("unknown")).toBe("none");
  });
});
