import { $ } from "../../app/dom";
import { appState } from "../../app/state";
import { CONNECTION_KIND_GROUPS } from "./connection-kinds";
import type { ConnectionKindOption } from "./connection-kinds";
import type { SourceEditorDialogController } from "./dialog";

export interface ConnectionChooserActions {
  openAddDialog: SourceEditorDialogController["openAddDialog"];
  openParentSettingsDialog: (parentKey: string) => Promise<void>;
}

export function filterConnectionKindGroups(query: string) {
  const normalized = query.trim().toLocaleLowerCase();
  if (!normalized) return CONNECTION_KIND_GROUPS;
  return CONNECTION_KIND_GROUPS.flatMap((group) => {
    const groupMatches = group.label.toLocaleLowerCase().includes(normalized);
    const options = groupMatches
      ? group.options
      : group.options.filter((option) => option.label.toLocaleLowerCase().includes(normalized));
    return options.length > 0 ? [{ ...group, options }] : [];
  });
}

export function mountConnectionChooser(actions: ConnectionChooserActions): void {
  const dialog = $<HTMLDialogElement>("connection-chooser");
  const listEl = $<HTMLDivElement>("connection-chooser-list");
  const filterInput = $<HTMLInputElement>("connection-chooser-filter");
  const filterClearBtn = $<HTMLButtonElement>("connection-chooser-filter-clear");
  const cancelBtn = $<HTMLButtonElement>("connection-chooser-cancel");
  const addBtn = $<HTMLButtonElement>("add");

  function renderList() {
    const groups = filterConnectionKindGroups(filterInput.value);
    filterClearBtn.hidden = filterInput.value.length === 0;
    listEl.innerHTML = "";
    if (groups.length === 0) {
      listEl.innerHTML = "<div class='picker-empty'>一致する接続先・テーブルはありません</div>";
      return;
    }
    for (const group of groups) {
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
  }

  filterInput.oninput = renderList;
  filterClearBtn.onclick = () => {
    filterInput.value = "";
    renderList();
    filterInput.focus();
  };
  renderList();

  cancelBtn.onclick = () => dialog.close();
  addBtn.onclick = () => {
    if (!dialog.open) {
      filterInput.value = "";
      renderList();
      dialog.showModal();
      filterInput.focus();
    }
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
