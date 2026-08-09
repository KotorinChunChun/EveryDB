import { describe, expect, it } from "vitest";

import { TYPE_DEFS } from "../../source-types/forms";
import { duplicateConfirmMessage } from "./source-identity";
import { CONNECTION_KIND_GROUPS, CONNECTION_KIND_OPTIONS, parentBrowseKind } from "./connection-kinds";

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

  it("groups Entra ID sources at the start of the Microsoft section", () => {
    const microsoft = CONNECTION_KIND_GROUPS.find((group) => group.category === "microsoft")!;
    expect(microsoft.options.slice(0, 3).map((option) => option.id)).toEqual([
      "graph_entra_users",
      "graph_entra_groups",
      "graph_entra_devices",
    ]);
  });
});

describe("source-identity", () => {
  it("builds duplicate confirm message in Japanese", () => {
    expect(duplicateConfirmMessage("ライセンス一覧")).toContain("ライセンス一覧");
    expect(duplicateConfirmMessage("ライセンス一覧")).toContain("再読込");
  });
});
