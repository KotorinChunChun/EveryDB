import { $, setPickerMessage } from "../../app/dom";
import { invoke } from "../../app/ipc";
import type { CredentialScope, CredentialSummary } from "../../shared/types";
import type { FieldDef } from "../../source-types/forms";
import { SECRET_PLACEHOLDER } from "../source-editor/constants";
import { getLinkedCredentials } from "../source-editor/linked-credentials";
import type { SourceEditorDialogController } from "../source-editor/dialog";
import type { CredentialManagerController } from "./manager";

export interface CredentialSelectorDeps {
  getSourceEditor: () => SourceEditorDialogController;
  manager: CredentialManagerController;
}

export interface CredentialSelectorController {
  wireCredentialSelector: (
    input: HTMLInputElement,
    f: FieldDef,
    idPrefix: string,
    row: HTMLElement,
    label: HTMLElement,
  ) => void;
}

function credentialScopeLabel(scope: CredentialScope): string {
  switch (scope) {
    case "notion": return "Notion";
    case "airtable": return "Airtable";
    case "graph": return "Microsoft 365 (Graph)";
    case "box": return "Box";
    case "postgres": return "PostgreSQL";
    case "mysql": return "MySQL";
  }
}

export function mountCredentialSelector(deps: CredentialSelectorDeps): CredentialSelectorController {
  const { getSourceEditor, manager } = deps;
  const { openCredentialManager, copyCredentialSecretToClipboard, copyCredentialBundleAsJson } = manager;

  let lastPickerScope: CredentialScope | null = null;

  function renderCredentialPickerList(
    items: CredentialSummary[],
    onPick: (id: number, label: string, fields: Record<string, unknown>) => void,
  ) {
    const listEl = $<HTMLDivElement>("credential-select-picker-list");
    listEl.innerHTML = "";
    if (items.length === 0) {
      listEl.innerHTML = "<div class='picker-empty'>該当する保存済みトークンがありません。下の「🔑 管理画面を開く…」から追加できます</div>";
      return;
    }
    for (const c of items) {
      const item = document.createElement("div");
      item.className = "picker-item";
      item.textContent = `${c.label}（作成: ${c.created_at}）`;
      item.onclick = async () => {
        try {
          const res = await invoke("credential_public_fields", { id: c.id });
          onPick(c.id, res.label, res.fields);
          $<HTMLDialogElement>("credential-select-picker").close();
        } catch (e) {
          alert("取得に失敗しました: " + String(e));
        }
      };
      listEl.appendChild(item);
    }
  }

  async function openCredentialSelectPicker(
    scope: CredentialScope,
    onPick: (id: number, label: string, fields: Record<string, unknown>) => void,
  ) {
    lastPickerScope = scope;
    const titleEl = $<HTMLHeadingElement>("credential-select-picker-title");
    titleEl.textContent = `保存済みトークンを選択（${credentialScopeLabel(scope)}）`;
    const listEl = $<HTMLDivElement>("credential-select-picker-list");
    listEl.innerHTML = "<div class='picker-empty'>読み込み中…</div>";
    const dlg = $<HTMLDialogElement>("credential-select-picker");

    dlg.showModal();
    try {
      const items = await invoke("list_credentials", { scope });
      renderCredentialPickerList(items, onPick);
    } catch (e) {
      setPickerMessage(listEl, `取得失敗: ${String(e)}`);
    }
  }

  $<HTMLButtonElement>("credential-select-picker-close").onclick = () => $<HTMLDialogElement>("credential-select-picker").close();
  $<HTMLButtonElement>("credential-select-picker-manage").onclick = () => {
    $<HTMLDialogElement>("credential-select-picker").close();
    void openCredentialManager(lastPickerScope ?? undefined);
  };

  function wireCredentialSelector(
    input: HTMLInputElement,
    f: FieldDef,
    idPrefix: string,
    row: HTMLElement,
    label: HTMLElement,
  ) {
    const scope = f.credentialScope!;
    const bundleFields = f.credentialBundleFields ?? [];
    input.hidden = true;
    label.textContent = "使用するトークン";

    const chip = document.createElement("span");
    chip.className = "credential-profile-chip";
    row.appendChild(chip);

    const pickBtn = document.createElement("button");
    pickBtn.type = "button";
    pickBtn.className = "btn browse-btn";
    row.appendChild(pickBtn);

    const copyBtn = document.createElement("button");
    copyBtn.type = "button";
    copyBtn.className = "btn browse-btn";
    copyBtn.textContent = "📋 コピー";
    copyBtn.title = "トークンをクリップボードにコピーします（本人確認を求められることがあります）";
    row.appendChild(copyBtn);

    function currentCredentialId(): number | null {
      if (input.dataset.credRef) return Number(input.dataset.credRef);
      const known = getLinkedCredentials()[f.key];
      return known ? known.id : null;
    }

    function applyPick(id: number, credLabel: string, fields: Record<string, unknown>) {
      input.dataset.credRef = String(id);
      input.dataset.credLabel = credLabel;
      input.value = SECRET_PLACEHOLDER;
      for (const bf of bundleFields) {
        const bfEl = document.getElementById(`${idPrefix}${bf}`) as HTMLInputElement | null;
        const v = fields[bf];
        if (bfEl && v !== undefined) bfEl.value = String(v);
      }
      renderState();
      const sourceEditor = getSourceEditor();
      if (f.key === "token" && idPrefix === "fd-" && $<HTMLSelectElement>("f-type").value === "airtable") {
        const baseId = (document.getElementById("fd-base_id") as HTMLInputElement | null)?.value.trim();
        const tableId = (document.getElementById("fd-table") as HTMLInputElement | null)?.value.trim();
        const viewId = (document.getElementById("fd-view") as HTMLInputElement | null)?.value.trim();
        if (baseId || tableId) void sourceEditor.resolveAirtableCaptions(baseId || undefined, tableId || undefined, viewId || undefined);
      }
      if (idPrefix === "fd-") {
        sourceEditor.maybeFetchAndRenderFields();
        sourceEditor.autoFillDefaultName();
      }
    }

    function renderState() {
      const lbl = input.dataset.credLabel;
      if (lbl) {
        chip.textContent = `🔑 ${lbl}`;
        chip.classList.remove("unset");
        pickBtn.textContent = "変更…";
        copyBtn.hidden = false;
      } else {
        chip.textContent = "未選択";
        chip.classList.add("unset");
        pickBtn.textContent = "選択…";
        copyBtn.hidden = true;
      }
    }

    pickBtn.onclick = () => openCredentialSelectPicker(scope, applyPick);
    copyBtn.onclick = () => {
      const id = currentCredentialId();
      if (id === null) return;
      if (bundleFields.length > 0) {
        void copyCredentialBundleAsJson(id, f.key, bundleFields, idPrefix);
      } else {
        void copyCredentialSecretToClipboard(id);
      }
    };

    const known = getLinkedCredentials()[f.key];
    if (known) input.dataset.credLabel = known.label;
    renderState();
  }

  return { wireCredentialSelector };
}
