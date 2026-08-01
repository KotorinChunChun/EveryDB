import { describe, expect, it } from "vitest";
import { SOURCE_TYPE_METADATA, guideNameForType, parentKindForType, representativeTypeForCredentialScope } from "./registry";

describe("source type metadata", () => {
  it("has unique IDs and guides for every registered type", () => {
    const ids = SOURCE_TYPE_METADATA.map((item) => item.id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(SOURCE_TYPE_METADATA.every((item) => item.guide.length > 0)).toBe(true);
  });

  it("keeps grouped and credential-bearing types on their expected contracts", () => {
    expect(parentKindForType("excel_table")).toBe("path");
    expect(parentKindForType("graph_sharepoint_list")).toBe("sharepoint");
    expect(guideNameForType("box_files")).toBe("box");
    expect(representativeTypeForCredentialScope("airtable")).toBe("airtable");
    expect(parentKindForType("unknown")).toBe("none");
  });
});