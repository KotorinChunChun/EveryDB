/** WCAG 2.x 相対輝度（sRGB）。 */
export function relativeLuminance(hex: string): number {
  const rgb = parseHex(hex);
  const [r, g, b] = rgb.map((channel) => {
    const s = channel / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** 前景/背景のコントラスト比（1:1 〜 21:1）。 */
export function contrastRatio(foreground: string, background: string): number {
  const l1 = relativeLuminance(foreground);
  const l2 = relativeLuminance(background);
  const lighter = Math.max(l1, l2);
  const darker = Math.min(l1, l2);
  return (lighter + 0.05) / (darker + 0.05);
}

function parseHex(hex: string): [number, number, number] {
  const normalized = hex.replace("#", "");
  if (normalized.length !== 6) throw new Error(`invalid hex: ${hex}`);
  return [
    Number.parseInt(normalized.slice(0, 2), 16),
    Number.parseInt(normalized.slice(2, 4), 16),
    Number.parseInt(normalized.slice(4, 6), 16),
  ];
}

export interface ThemeTokenSet {
  bg: string;
  text: string;
  muted: string;
  accent: string;
  onAccent: string;
  danger: string;
  onDanger: string;
  formError: string;
  errorStateBg: string;
  errorStateText: string;
}

/** `tokens.css` と同期したテーマ色（監査用）。 */
export const THEME_TOKENS: Record<"dark" | "light" | "night", ThemeTokenSet> = {
  dark: {
    bg: "#1e1e1e",
    text: "#e0e0e0",
    muted: "#9a9a9a",
    accent: "#0e639c",
    onAccent: "#ffffff",
    danger: "#5a1d1d",
    onDanger: "#ffffff",
    formError: "#f85149",
    errorStateBg: "#3a1518",
    errorStateText: "#ffd7d9",
  },
  light: {
    bg: "#ffffff",
    text: "#1c1c1c",
    muted: "#5f5f5f",
    accent: "#0b5c94",
    onAccent: "#ffffff",
    danger: "#c62828",
    onDanger: "#ffffff",
    formError: "#b42318",
    errorStateBg: "#fde7e9",
    errorStateText: "#7d1117",
  },
  night: {
    bg: "#000000",
    text: "#ffffff",
    muted: "#b0b0b0",
    accent: "#3399ff",
    onAccent: "#000000",
    danger: "#b33333",
    onDanger: "#ffffff",
    formError: "#ff6b6b",
    errorStateBg: "#260000",
    errorStateText: "#ffffff",
  },
};
