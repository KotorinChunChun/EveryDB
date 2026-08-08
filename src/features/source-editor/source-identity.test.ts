import { describe, expect, it } from "vitest";

import { duplicateConfirmMessage } from "./source-identity";
import { CONNECTION_KIND_OPTIONS, parentBrowseKind } from "./connection-kinds";

describe("connection-kinds", () => {
  it("maps representative types to parent browse kinds", () => {
    expect(parentBrowseKind("graph_entra_users")).toBe("graph365");
    expect(parentBrowseKind("graph_sharepoint_list")).toBe("sharepoint");
    expect(parentBrowseKind("csv")).toBeNull();
  });

  it("includes standalone table option", () => {
    const standalone = CONNECTION_KIND_OPTIONS.find((k) => k.standalone);
    expect(standalone?.label).toContain("単体テーブル");
  });
});

describe("source-identity", () => {
  it("builds duplicate confirm message in Japanese", () => {
    expect(duplicateConfirmMessage("ライセンス一覧")).toContain("ライセンス一覧");
    expect(duplicateConfirmMessage("ライセンス一覧")).toContain("再読込");
  });
});
