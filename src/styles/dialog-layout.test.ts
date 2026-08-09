// @ts-expect-error Vitest runs in Node, while the application tsconfig intentionally omits Node globals.
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const dialogsCss = readFileSync("src/styles/dialogs.css", "utf8");

describe("dialog layout CSS", () => {
  it("keeps the flex-based source editor hidden after dialog.close()", () => {
    expect(dialogsCss).toMatch(/#add-dialog:not\(\[open\]\)\s*\{\s*display:\s*none;\s*\}/);
  });
});
