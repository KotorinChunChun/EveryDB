import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { FieldDef } from "../../source-types/forms";
import { makeApiKeyButton, makeLinkButton } from "./ui-buttons";

export interface FieldRendererActions {
  wireCredentialSelector: (
    input: HTMLInputElement,
    field: FieldDef,
    idPrefix: string,
    row: HTMLElement,
    label: HTMLElement,
    sourceType: string,
  ) => void;
  openAirtableBasePicker: (input: HTMLInputElement) => void;
  openAirtableTablePicker: (input: HTMLInputElement) => void;
  openAirtableViewPicker: (input: HTMLInputElement) => void;
}

/** テキスト/パスワード系フィールドを任意のコンテナへ描画する（接続先ダイアログ等向け）。 */
export function renderFieldsInto(
  container: HTMLElement,
  idPrefix: string,
  fields: FieldDef[],
  actions: FieldRendererActions,
  sourceType: string,
  prefill?: Record<string, unknown>,
): void {
  container.innerHTML = "";
  const credentialManagedKeys = new Set<string>();
  for (const field of fields) {
    if (field.credentialScope) {
      credentialManagedKeys.add(field.key);
      for (const bundleField of field.credentialBundleFields ?? []) credentialManagedKeys.add(bundleField);
    }
  }
  for (const field of fields) {
    const isCredOwner = !!field.credentialScope;
    const isCredManagedSibling = !isCredOwner && credentialManagedKeys.has(field.key);
    const label = document.createElement("label");
    label.textContent = field.label;
    const input = document.createElement("input");
    input.type = field.kind === "password" ? "password" : "text";
    input.autocomplete = field.kind === "password" ? "off" : "on";
    input.id = `${idPrefix}${field.key}`;
    if (field.placeholder && !isCredManagedSibling) input.placeholder = field.placeholder;
    if (prefill) {
      const value = prefill[field.key];
      if (value !== undefined && value !== null) input.value = String(value);
    }
    if (isCredManagedSibling) {
      input.hidden = true;
      container.appendChild(input);
      continue;
    }
    if (
      isCredOwner
      || field.browse
      || field.helpUrl
      || field.linkUrl
      || field.browseAirtableBase
      || field.browseAirtableTable
      || field.browseAirtableView
    ) {
      const row = document.createElement("div");
      row.className = "path-row";
      row.appendChild(input);
      if (field.browse) {
        const browseBtn = document.createElement("button");
        browseBtn.type = "button";
        browseBtn.className = "btn browse-btn";
        browseBtn.textContent = "参照…";
        browseBtn.onclick = async () => {
          try {
            const picked = await openDialog({
              directory: field.browse === "folder",
              multiple: false,
              filters: field.browse === "file" ? field.dialogFilters : undefined,
            });
            if (typeof picked === "string") input.value = picked;
          } catch (e) {
            console.error("ファイル選択に失敗:", e);
          }
        };
        row.appendChild(browseBtn);
      }
      if (isCredOwner) actions.wireCredentialSelector(input, field, idPrefix, row, label, sourceType);
      if (field.browseAirtableBase) {
        const pickBtn = document.createElement("button");
        pickBtn.type = "button";
        pickBtn.className = "btn browse-btn";
        pickBtn.textContent = "一覧から選択…";
        pickBtn.title = "Personal Access Token を使用して利用可能な Base 一覧を取得します";
        pickBtn.onclick = () => actions.openAirtableBasePicker(input);
        row.appendChild(pickBtn);
      }
      if (field.browseAirtableTable) {
        const pickBtn = document.createElement("button");
        pickBtn.type = "button";
        pickBtn.className = "btn browse-btn";
        pickBtn.textContent = "一覧から選択…";
        pickBtn.title = "指定された Base 内のテーブル一覧を取得します";
        pickBtn.onclick = () => actions.openAirtableTablePicker(input);
        row.appendChild(pickBtn);
      }
      if (field.browseAirtableView) {
        const pickBtn = document.createElement("button");
        pickBtn.type = "button";
        pickBtn.className = "btn browse-btn";
        pickBtn.textContent = "一覧から選択…";
        pickBtn.title = "指定されたテーブル内のビュー一覧を取得します";
        pickBtn.onclick = () => actions.openAirtableViewPicker(input);
        row.appendChild(pickBtn);
      }
      if (!isCredOwner && field.helpUrl) row.appendChild(makeApiKeyButton(field.helpUrl));
      if (!isCredOwner && field.linkUrl) row.appendChild(makeLinkButton(field.linkUrl, field.linkLabel ?? "🌐 開く…"));
      label.appendChild(row);
    } else {
      label.appendChild(input);
    }
    container.appendChild(label);
  }
}
