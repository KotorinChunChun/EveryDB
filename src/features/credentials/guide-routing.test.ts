import { describe, expect, it } from "vitest";

import { GRAPH_CAPABILITY_OPTIONS, graphCapabilitiesFromFields, graphGuideDocument } from "./manager";

describe("Graph credential guide routing", () => {
  it("routes each authentication method to its dedicated guide", () => {
    expect(graphGuideDocument("app_only")).toBe("microsoft365-entra-app");
    expect(graphGuideDocument("delegated")).toBe("microsoft365-entra-user");
  });

  it("keeps legacy delegated credentials on the SharePoint capability", () => {
    expect(graphCapabilitiesFromFields({ auth_method: "delegated" })).toEqual(["sharepoint"]);
  });

  it("keeps only supported stored capabilities", () => {
    expect(graphCapabilitiesFromFields({
      auth_method: "delegated",
      capabilities: ["onedrive_self", "unknown", "directory_users", "licenses"],
    })).toEqual(["onedrive_self", "directory_users", "licenses"]);
  });

  it("offers least-privilege capabilities for all Entra ID group sources", () => {
    expect(GRAPH_CAPABILITY_OPTIONS.slice(0, 5)).toEqual([
      { value: "directory_users", label: "Entra ID ユーザー一覧", scopes: "User.Read.All" },
      { value: "directory_groups", label: "Entra ID グループ一覧", scopes: "Group.Read.All" },
      { value: "directory_devices", label: "Entra ID デバイス一覧", scopes: "Device.Read.All" },
      { value: "intune_devices", label: "Intune デバイス一覧", scopes: "DeviceManagementManagedDevices.Read.All" },
      { value: "licenses", label: "Microsoft 365 ライセンス一覧", scopes: "LicenseAssignment.Read.All" },
    ]);
  });
});
