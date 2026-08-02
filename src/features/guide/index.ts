import { $ } from "../../app/dom";
import { guideNameForType } from "../../source-types/registry";
import { renderMarkdownInto } from "./markdown";

const GUIDE_DOCS = import.meta.glob("/docs/guides/*.md", { query: "?raw", import: "default", eager: true }) as Record<string, string>;

export interface GuideViewerController {
  guideTextFor: (type_: string) => string | null;
  openGuide: (type_: string) => void;
}

/** 接続先種別ごとの手順書ダイアログとショートカット一覧をマウントする。 */
export function mountGuideViewer(): GuideViewerController {
  const guideDialog = $<HTMLDialogElement>("guide-dialog");
  const guideBody = $<HTMLDivElement>("guide-body");

  function guideTextFor(type_: string): string | null {
    const name = guideNameForType(type_);
    return name ? GUIDE_DOCS[`/docs/guides/${name}.md`] ?? null : null;
  }

  function openGuide(type_: string) {
    const md = guideTextFor(type_);
    if (!md) return;
    const firstH1 = /^#\s+(.*)$/m.exec(md);
    $<HTMLHeadingElement>("guide-title").textContent = firstH1 ? firstH1[1] : "接続手順書";
    renderMarkdownInto(guideBody, firstH1 ? md.replace(firstH1[0], "") : md);
    guideBody.scrollTop = 0;
    guideDialog.showModal();
  }

  $<HTMLButtonElement>("guide-close").onclick = () => guideDialog.close();
  $<HTMLButtonElement>("guide-close-x").onclick = () => guideDialog.close();

  $<HTMLButtonElement>("source-guide-btn")?.addEventListener("click", () => {
    $<HTMLDialogElement>("shortcut-guide-dialog")?.showModal();
  });
  $<HTMLButtonElement>("shortcut-guide-close")?.addEventListener("click", () => {
    $<HTMLDialogElement>("shortcut-guide-dialog")?.close();
  });
  $<HTMLButtonElement>("shortcut-guide-close-x")?.addEventListener("click", () => {
    $<HTMLDialogElement>("shortcut-guide-dialog")?.close();
  });

  return { guideTextFor, openGuide };
}
