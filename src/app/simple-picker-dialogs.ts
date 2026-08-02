export interface SimplePickerDialogDef {
  dialogId: string;
  titleId: string;
  title: string;
  listId: string;
  closeId: string;
}

/** 単純なリスト + 閉じるボタン構成の picker 定義（RF-30-62）。 */
export const SIMPLE_PICKER_DIALOGS: readonly SimplePickerDialogDef[] = [
  {
    dialogId: "notion-picker",
    titleId: "notion-picker-title",
    title: "Notion データベースを選択",
    listId: "notion-picker-list",
    closeId: "notion-picker-close",
  },
  {
    dialogId: "browser-profile-picker",
    titleId: "browser-profile-picker-title",
    title: "プロファイルを選択",
    listId: "browser-profile-picker-list",
    closeId: "browser-profile-picker-close",
  },
  {
    dialogId: "sheet-picker",
    titleId: "sheet-picker-title",
    title: "シートを選択",
    listId: "sheet-picker-list",
    closeId: "sheet-picker-close",
  },
  {
    dialogId: "airtable-picker",
    titleId: "airtable-picker-title",
    title: "Airtable 項目を選択",
    listId: "airtable-picker-list",
    closeId: "airtable-picker-close",
  },
] as const;

/** `#simple-picker-dialog-template` から重複 picker DOM を生成する。 */
export function mountSimplePickerDialogs(): void {
  const template = document.getElementById("simple-picker-dialog-template") as HTMLTemplateElement | null;
  if (!template) throw new Error("simple-picker-dialog-template missing");

  for (const def of SIMPLE_PICKER_DIALOGS) {
    if (document.getElementById(def.dialogId)) continue;

    const fragment = template.content.cloneNode(true) as DocumentFragment;
    const dialog = fragment.querySelector("dialog");
    const title = fragment.querySelector("h3");
    const list = fragment.querySelector(".picker-list");
    const closeBtn = fragment.querySelector(".dialog-actions .btn");
    if (!dialog || !title || !list || !closeBtn) {
      throw new Error("simple-picker-dialog-template structure invalid");
    }

    dialog.id = def.dialogId;
    dialog.setAttribute("aria-labelledby", def.titleId);
    title.id = def.titleId;
    title.textContent = def.title;
    list.id = def.listId;
    closeBtn.id = def.closeId;

    document.body.appendChild(fragment);
  }
}
