import { describe, expect, it } from "vitest";

import { graphGuideDocument } from "./manager";

describe("Graph credential guide routing", () => {
  it("routes each authentication method to its dedicated guide", () => {
    expect(graphGuideDocument("app_only")).toBe("microsoft365-app-only");
    expect(graphGuideDocument("delegated")).toBe("microsoft365-delegated");
  });
});