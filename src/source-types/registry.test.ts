import { describe, expect, it } from "vitest";
import { SOURCE_TYPE_MANIFEST, SOURCE_TYPE_METADATA, guideNameForType, parentKindForType, representativeTypeForCredentialScope } from "./registry";

describe("source type metadata", () => {
  it("has unique, complete metadata for every registered type", () => {
    const ids = SOURCE_TYPE_METADATA.map((item) => item.id);
    const parentKinds = new Set(["path", "sharepoint", "notion", "airtable", "graph365", "postgres", "mysql", "box", "none"]);
    const categories = new Set(["local", "microsoft", "cloud", "database", "public-data", "web"]);

    expect(new Set(ids).size).toBe(ids.length);
    expect(SOURCE_TYPE_METADATA.every((item) => item.label.trim().length > 0 && item.guide.length > 0)).toBe(true);
    expect(SOURCE_TYPE_METADATA.every((item) => parentKinds.has(item.parentKind) && categories.has(item.category))).toBe(true);
  });

  it("keeps the legacy Excel cell type out of new source selection", () => {
    expect(SOURCE_TYPE_MANIFEST.find((item) => item.id === "excel_cell")).toMatchObject({ frontend: false });
    expect(SOURCE_TYPE_METADATA.some((item) => item.id === "excel_cell")).toBe(false);
  });

  it("keeps grouped and credential-bearing types on their expected contracts", () => {
    expect(parentKindForType("excel_table")).toBe("path");
    expect(parentKindForType("graph_sharepoint_list")).toBe("sharepoint");
    expect(guideNameForType("box_files")).toBe("box");
    expect(representativeTypeForCredentialScope("airtable")).toBe("airtable");
    expect(parentKindForType("unknown")).toBe("none");
  });
});