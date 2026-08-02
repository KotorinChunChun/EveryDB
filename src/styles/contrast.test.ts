import { describe, expect, it } from "vitest";

import { contrastRatio, THEME_TOKENS } from "./theme-tokens";

const AA_NORMAL = 4.5;
const AA_LARGE = 3;

describe("theme token contrast (RF-30-61)", () => {
  for (const [theme, tokens] of Object.entries(THEME_TOKENS)) {
    describe(theme, () => {
      it("keeps body text readable on background", () => {
        expect(contrastRatio(tokens.text, tokens.bg)).toBeGreaterThanOrEqual(AA_NORMAL);
      });

      it("keeps secondary text readable on background", () => {
        expect(contrastRatio(tokens.muted, tokens.bg)).toBeGreaterThanOrEqual(AA_NORMAL);
      });

      it("keeps accent button labels readable", () => {
        expect(contrastRatio(tokens.onAccent, tokens.accent)).toBeGreaterThanOrEqual(AA_NORMAL);
      });

      it("keeps danger button labels readable", () => {
        expect(contrastRatio(tokens.onDanger, tokens.danger)).toBeGreaterThanOrEqual(AA_NORMAL);
      });

      it("keeps form error text readable on background", () => {
        expect(contrastRatio(tokens.formError, tokens.bg)).toBeGreaterThanOrEqual(AA_NORMAL);
      });
    });
  }

  it("documents night mode as high-contrast theme", () => {
    expect(contrastRatio(THEME_TOKENS.night.text, THEME_TOKENS.night.bg)).toBeGreaterThanOrEqual(AA_LARGE);
  });
});
