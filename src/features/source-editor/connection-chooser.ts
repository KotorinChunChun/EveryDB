import { $ } from "../../app/dom";
import { appState } from "../../app/state";
import { CONNECTION_KIND_GROUPS } from "./connection-kinds";
import type { ConnectionKindOption } from "./connection-kinds";
import type { SourceEditorDialogController } from "./dialog";

export interface ConnectionChooserActions {
  openAddDialog: SourceEditorDialogController["openAddDialog"];
  openParentSettingsDialog: (parentKey: string) => Promise<void>;
}

export function mountConnectionChooser(actions: ConnectionChooserActions): void {
  const dialog = $<HTMLDialogElement>("connection-chooser");
  const listEl = $<HTMLDivElement>("connection-chooser-list");
  const cancelBtn = $<HTMLButtonElement>("connection-chooser-cancel");
  const addBtn = $<HTMLButtonElement>("add");

  listEl.innerHTML = "";
  for (const group of CONNECTION_KIND_GROUPS) {
    const heading = document.createElement("div");
    heading.className = "connection-kind-group";
    heading.textContent = group.label;
    listEl.appendChild(heading);

    for (const kind of group.options) {
      const row = document.createElement("button");
      row.type = "button";
      row.className = "picker-item connection-kind-item";
      row.textContent = kind.label;
      row.onclick = () => {
        dialog.close();
        void handleKindSelect(kind, actions);
      };
      listEl.appendChild(row);
    }
  }

  cancelBtn.onclick = () => dialog.close();
  addBtn.onclick = () => {
    if (!dialog.open) dialog.showModal();
  };
}

async function handleKindSelect(
  kind: ConnectionKindOption,
  actions: ConnectionChooserActions,
): Promise<void> {
  if (kind.openExistingGraphParent) {
    const existing = appState.sources.some((s) => s.parent === "Microsoft 365");
    if (existing) {
      await actions.openParentSettingsDialog("Microsoft 365");
      return;
    }
  }

  actions.openAddDialog({
    type: kind.repType,
    title: kind.connectionAdd ? "接続先を追加" : "テーブル追加",
    connectionAdd: kind.connectionAdd,
  });
}
