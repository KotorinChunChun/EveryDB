import { describe, expect, it } from "vitest";
import {
  applyColumnOrder,
  collectRoleConfigFromTable,
  configWithoutRoleKeys,
  roleStateFromConfig,
} from "./roles";

describe("applyColumnOrder", () => {
  it("preserves saved order and appends new fields", () => {
    expect(applyColumnOrder(["c", "a", "b"], ["b", "a"])).toEqual(["b", "a", "c"]);
  });
});

describe("roleStateFromConfig", () => {
  it("extracts role fields", () => {
    const state = roleStateFromConfig({
      label_column: "name",
      searchable_columns: ["name"],
      display_columns: ["name", "id"],
      column_aliases: { id: "ID" },
      column_order: ["name", "id"],
    });
    expect(state.label_column).toBe("name");
    expect(state.searchable.has("name")).toBe(true);
    expect(state.aliases.id).toBe("ID");
    expect(state.order).toEqual(["name", "id"]);
  });
});

describe("configWithoutRoleKeys", () => {
  it("removes role keys only", () => {
    expect(
      configWithoutRoleKeys({ path: "/x", label_column: "a", display_columns: ["a"] }),
    ).toEqual({ path: "/x" });
  });
});

describe("collectRoleConfigFromTable", () => {
  it("reads checkbox and alias values from rows", () => {
    document.body.innerHTML = `
      <table><tbody id="rows">
        <tr data-name="name">
          <td></td><td></td><td></td><td></td><td></td><td></td>
          <td><input class="r-display" type="checkbox" checked></td>
          <td><input class="r-search" type="checkbox" checked></td>
          <td><input name="r-label" type="radio" checked></td>
          <td><input name="r-openref" type="radio"></td>
          <td><input class="r-alias" value="表示名"></td>
        </tr>
      </tbody></table>`;
    const rowsEl = document.getElementById("rows") as HTMLTableSectionElement;
    const config = collectRoleConfigFromTable(rowsEl, ["name"]);
    expect(config.label_column).toBe("name");
    expect(config.column_aliases).toEqual({ name: "表示名" });
    expect(config.column_order).toEqual(["name"]);
  });
});
