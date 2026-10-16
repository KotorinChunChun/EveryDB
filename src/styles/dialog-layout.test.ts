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

  it("keeps picker search controls at their natural height and gives the remaining height to lists", () => {
    expect(dialogsCss).toMatch(/\.stable-picker-dialog \.search,\s*\.stable-picker-dialog \.source-filter-wrap\s*\{\s*flex:\s*none;/s);
    expect(dialogsCss).toMatch(/\.stable-picker-dialog > \.picker-list,[^{]+\{[^}]*flex:\s*1;/s);
    expect(dialogsCss).toMatch(/\.stable-picker-dialog \.sp-step > \.picker-list,[^{]+\{[^}]*flex:\s*1;/s);
  });

  it("contains source editor controls inside a scrollable fieldset at narrow sizes", () => {
    expect(dialogsCss).toMatch(/\.dialog\.wide\s*\{[^}]*width:\s*min\(560px,\s*calc\(100vw - 32px\)\);[^}]*min-width:\s*min\(560px,\s*calc\(100vw - 32px\)\);/s);
    expect(dialogsCss).toMatch(/#add-dialog \.roles-fieldset\s*\{[^}]*min-width:\s*0;[^}]*min-height:\s*0;[^}]*max-width:\s*100%;/s);
    expect(dialogsCss).toMatch(/#add-dialog \.roles-fieldset:not\(\.config-fieldset\)\s*\{[^}]*min-height:\s*96px;[^}]*overflow:\s*auto;/s);
    expect(dialogsCss).toMatch(/\.roles-toolbar\s*\{[^}]*flex:\s*none;[^}]*flex-wrap:\s*wrap;/s);
  });
});
