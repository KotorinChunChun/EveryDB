import { $ } from "../../app/dom";
import { appState } from "../../app/state";
import { CONNECTION_KIND_OPTIONS } from "./connection-kinds";
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
  for (const kind of CONNECTION_KIND_OPTIONS) {
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

  cancelBtn.onclick = () => dialog.close();
  addBtn.onclick = () => {
    if (!dialog.open) dialog.showModal();
  };
}

async function handleKindSelect(
  kind: typeof CONNECTION_KIND_OPTIONS[number],
  actions: ConnectionChooserActions,
): Promise<void> {
  if (kind.id === "graph365") {
    const existing = appState.sources.some((s) => s.parent === "Microsoft 365");
    if (existing) {
      await actions.openParentSettingsDialog("Microsoft 365");
      return;
    }
  }

  if (kind.standalone) {
    actions.openAddDialog({ title: "テーブル追加", connectionAdd: false });
    return;
  }

  actions.openAddDialog({
    type: kind.repType,
    title: "接続先を追加",
    connectionAdd: true,
  });
}
