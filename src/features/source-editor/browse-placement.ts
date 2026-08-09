import type { TypeDef } from "../../source-types/forms";

export type BrowsePlacement = "auto-setup" | "separate" | "none";

/** 種別ごとの参照ボタンを、フォーム内のどの導線へ置くか一元管理する。 */
export function browsePlacement(
  definition: TypeDef,
  dialogMode: "add" | "edit",
): BrowsePlacement {
  if (dialogMode !== "add" || !definition.browse) return "none";
  return definition.browse === "sharepoint" || definition.browse === "notion"
    ? "auto-setup"
    : "separate";
}
