import { describe, expect, it } from "vitest";
import { SECRET_PLACEHOLDER } from "../source-editor/constants";
import { clearLinkedCredentials, getLinkedCredentials, setLinkedCredentials } from "../source-editor/linked-credentials";

describe("credential secret retention", () => {
  it("stores only id and label in linked credentials, never secret values", () => {
    clearLinkedCredentials();
    setLinkedCredentials({
      token: { id: 42, label: "Work Notion" },
      client_secret: { id: 7, label: "Graph prod" },
    });

    const linked = getLinkedCredentials();
    expect(Object.keys(linked)).toEqual(["token", "client_secret"]);
    for (const entry of Object.values(linked)) {
      expect(Object.keys(entry).sort()).toEqual(["id", "label"]);
      expect(entry).not.toHaveProperty("value");
      expect(entry).not.toHaveProperty("secret");
      expect(entry).not.toHaveProperty("fields");
    }
  });

  it("uses a fixed placeholder that is not a plausible real secret", () => {
    expect(SECRET_PLACEHOLDER).toBe("••••••••");
    expect(SECRET_PLACEHOLDER).not.toMatch(/^[A-Za-z0-9_-]{16,}$/);
    expect(SECRET_PLACEHOLDER.length).toBeLessThan(12);
  });
});
