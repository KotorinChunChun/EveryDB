import { describe, expect, it } from "vitest";
import { TYPE_DEFS } from "../../source-types/forms";
import { browsePlacement } from "./browse-placement";

describe("source editor browse button placement", () => {
  it("places the Notion database picker beside URL auto input", () => {
    const notion = TYPE_DEFS.find((definition) => definition.value === "notion")!;
    expect(browsePlacement(notion, "add")).toBe("auto-setup");
  });

  it("keeps file/table pickers in the separate browse area", () => {
    const sqlite = TYPE_DEFS.find((definition) => definition.value === "sqlite")!;
    expect(browsePlacement(sqlite, "add")).toBe("separate");
  });

  it("hides source selection pickers while editing an existing source", () => {
    const notion = TYPE_DEFS.find((definition) => definition.value === "notion")!;
    expect(browsePlacement(notion, "edit")).toBe("none");
  });
});
