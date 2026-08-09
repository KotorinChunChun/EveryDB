import { describe, expect, it } from "vitest";

import { TYPE_DEFS } from "../../source-types/forms";
import { duplicateConfirmMessage } from "./source-identity";
import { CONNECTION_KIND_OPTIONS, parentBrowseKind } from "./connection-kinds";

describe("connection-kinds", () => {
  it("maps representative types to parent browse kinds", () => {
    expect(parentBrowseKind("graph_entra_users")).toBe("graph365");
    expect(parentBrowseKind("graph_sharepoint_list")).toBe("sharepoint");
    expect(parentBrowseKind("csv")).toBeNull();
  });

  it("lists every addable table type directly", () => {
    expect(CONNECTION_KIND_OPTIONS.map((option) => option.repType).sort())
      .toEqual(TYPE_DEFS.map((definition) => definition.value).sort());
    expect(CONNECTION_KIND_OPTIONS.find((option) => option.repType === "csv")).toMatchObject({
      label: "CSV ファイル",
      connectionAdd: false,
    });
    expect(CONNECTION_KIND_OPTIONS.find((option) => option.repType === "graph_onedrive")).toMatchObject({
      label: "OneDrive（組織）",
      openExistingGraphParent: false,
    });
    expect(CONNECTION_KIND_OPTIONS.find((option) => option.repType === "microsoft_account_onedrive")).toMatchObject({
      label: "OneDrive（個人）",
      connectionAdd: false,
    });
  });
});

describe("source-identity", () => {
  it("builds duplicate confirm message in Japanese", () => {
    expect(duplicateConfirmMessage("ライセンス一覧")).toContain("ライセンス一覧");
    expect(duplicateConfirmMessage("ライセンス一覧")).toContain("再読込");
  });
});
