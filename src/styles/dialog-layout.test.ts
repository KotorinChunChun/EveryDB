// @ts-expect-error Vitest runs in Node, while the application tsconfig intentionally omits Node globals.
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const dialogsCss = readFileSync("src/styles/dialogs.css", "utf8");

describe("dialog layout CSS", () => {
  it("keeps the flex-based source editor hidden after dialog.close()", () => {
    expect(dialogsCss).toMatch(/#add-dialog:not\(\[open\]\)\s*\{\s*display:\s*none;\s*\}/);
  });

  it("keeps picker dimensions stable while allowing viewport-bounded user resize", () => {
    expect(dialogsCss).toMatch(/\.dialog\.stable-picker-dialog\s*\{[^}]*width:\s*min\([^;]+;[^}]*height:\s*min\([^;]+;/s);
    expect(dialogsCss).toMatch(/\.dialog\.stable-picker-dialog\s*\{[^}]*max-width:\s*calc\(100vw - 32px\);[^}]*max-height:\s*calc\(100vh - 32px\);/s);
    expect(dialogsCss).toMatch(/\.dialog\.stable-picker-dialog\s*\{[^}]*resize:\s*both;/s);
    expect(dialogsCss).toMatch(/\.dialog\.stable-picker-dialog:not\(\[open\]\)\s*\{\s*display:\s*none;/s);
  });
});
