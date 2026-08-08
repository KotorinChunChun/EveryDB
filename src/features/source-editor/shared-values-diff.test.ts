import { describe, expect, it } from "vitest";

import { sharedConfigWouldChange, sharedKeyValueChanged } from "./shared-values-diff";
import { SECRET_PLACEHOLDER } from "./constants";

describe("shared-values-diff", () => {
  const norm = (v: string | null | undefined) => (v ?? "").trim();

  it("treats SECRET_PLACEHOLDER as unchanged", () => {
    expect(sharedKeyValueChanged("client_secret", SECRET_PLACEHOLDER, "old", norm)).toBe(false);
  });

  it("detects path and cred ref changes", () => {
    expect(sharedKeyValueChanged("path", "C:\\a.db", "C:\\b.db", norm)).toBe(true);
    expect(sharedKeyValueChanged("token", "cred:1", "cred:2", norm)).toBe(true);
    expect(sharedKeyValueChanged("token", "cred:1", "cred:1", norm)).toBe(false);
  });

  it("skips optional keys omitted from sharedValues", () => {
    const existing = { tenant_id: "t1", client_id: "c1" };
    expect(sharedConfigWouldChange(existing, { tenant_id: "t1" }, ["tenant_id", "client_id"], norm)).toBe(false);
    expect(sharedConfigWouldChange(existing, { tenant_id: "t2" }, ["tenant_id", "client_id"], norm)).toBe(true);
  });
});
