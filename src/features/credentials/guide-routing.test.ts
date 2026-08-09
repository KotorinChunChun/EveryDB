import { describe, expect, it } from "vitest";

import { graphCapabilitiesFromFields, graphGuideDocument } from "./manager";

describe("Graph credential guide routing", () => {
  it("routes each authentication method to its dedicated guide", () => {
    expect(graphGuideDocument("app_only")).toBe("microsoft365-app-only");
    expect(graphGuideDocument("delegated")).toBe("microsoft365-delegated");
  });

  it("keeps legacy delegated credentials on the SharePoint capability", () => {
    expect(graphCapabilitiesFromFields({ auth_method: "delegated" })).toEqual(["sharepoint"]);
  });

  it("keeps only supported stored capabilities", () => {
    expect(graphCapabilitiesFromFields({
      auth_method: "delegated",
      capabilities: ["onedrive_self", "unknown", "calendar_self"],
    })).toEqual(["onedrive_self", "calendar_self"]);
  });
});
