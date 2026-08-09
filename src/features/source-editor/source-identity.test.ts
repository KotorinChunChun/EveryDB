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

  it("separates Microsoft sources into exact Entra ID and Microsoft 365 groups", () => {
    const entra = CONNECTION_KIND_GROUPS.find((group) => group.id === "entra")!;
    const microsoft = CONNECTION_KIND_GROUPS.find((group) => group.id === "microsoft")!;

    expect(entra.label).toBe("Entra ID");
    expect(entra.options.map((option) => option.id)).toEqual([
      "graph_entra_users",
      "graph_entra_groups",
      "graph_entra_devices",
      "graph_intune_devices",
      "graph_licenses",
    ]);
    expect(microsoft.label).toBe("Microsoft 365");
    expect(microsoft.options.map((option) => option.id)).toEqual([
      "graph_teams",
      "graph_sharepoint_list",
      "graph_sharepoint_library",
      "graph_calendar",
      "graph_onedrive",
      "microsoft_account_onedrive",
      "graph_planner",
    ]);

    const groupedIds = CONNECTION_KIND_GROUPS.flatMap((group) => group.options.map((option) => option.id));
    expect(groupedIds).toHaveLength(CONNECTION_KIND_OPTIONS.length);
    expect(new Set(groupedIds).size).toBe(CONNECTION_KIND_OPTIONS.length);
  });

  it("uses the official Entra ID spelling in source labels", () => {
    expect(CONNECTION_KIND_OPTIONS.filter((option) => option.id.startsWith("graph_entra"))
      .map((option) => option.label)).toEqual([
      "Entra ID ユーザー一覧",
      "Entra ID グループ一覧",
      "Entra ID デバイス一覧",
    ]);
  });
});

describe("source-identity", () => {
  it("builds duplicate confirm message in Japanese", () => {
    expect(duplicateConfirmMessage("ライセンス一覧")).toContain("ライセンス一覧");
    expect(duplicateConfirmMessage("ライセンス一覧")).toContain("再読込");
  });
});
