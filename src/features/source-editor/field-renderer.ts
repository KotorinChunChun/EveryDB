import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type { FieldDef } from "../../source-types/forms";
import { makeApiKeyButton, makeLinkButton } from "./ui-buttons";

export type WireCredentialSelector = (
  input: HTMLInputElement,
  field: FieldDef,
  idPrefix: string,
  row: HTMLElement,
  label: HTMLElement,
  sourceType: string,
) => void;

/** 入力欄の横に置く一覧選択ボタンの呼び出し先。描画先ごとに対応するものだけを渡す。 */
export interface FieldPickerActions {
  openBrowserProfilePicker: (target: "history" | "bookmarks", input: HTMLInputElement) => void;
  openSheetPicker: (input: HTMLInputElement) => void;
  openExcelTablePicker: (input: HTMLInputElement) => void;
  openCellPicker: (
    input: HTMLInputElement,
    gridMode: boolean | "top" | "bottom" | "bottom_left" | "bottom_right",
    pickMode: "cell" | "column",
  ) => void;
  openAirtableBasePicker: (input: HTMLInputElement) => void;
  openAirtableTablePicker: (input: HTMLInputElement) => void;
  openAirtableViewPicker: (input: HTMLInputElement) => void;
}

export interface FieldRendererActions
  extends Pick<FieldPickerActions, "openAirtableBasePicker" | "openAirtableTablePicker" | "openAirtableViewPicker"> {
  wireCredentialSelector: WireCredentialSelector;
}

export interface FieldActionRowOptions {
  idPrefix: string;
  sourceType: string;
  label: HTMLElement;
  wireCredentialSelector: WireCredentialSelector;
  pickers: Partial<FieldPickerActions>;
  /** 「参照…」でファイル/フォルダを選んで入力欄へ反映した直後に呼ぶ。 */
  onFilePicked?: () => void;
}

/** 秘密フィールド本体と、同じトークン選択で埋まるバンドルフィールド（tenant_id 等）のキー。 */
export function credentialManagedKeysOf(fields: FieldDef[]): Set<string> {
  const keys = new Set<string>();
  for (const field of fields) {
    if (field.credentialScope) {
      keys.add(field.key);
      for (const bundleField of field.credentialBundleFields ?? []) keys.add(bundleField);
    }
  }
  return keys;
}

function makeRowButton(text: string, title: string | null, onclick: () => void): HTMLButtonElement {
  const btn = document.createElement("button");
  btn.type = "button";
  btn.className = "btn browse-btn";
  btn.textContent = text;
  if (title !== null) btn.title = title;
  btn.onclick = onclick;
  return btn;
}

/** ファイル/フォルダ選択ダイアログを開き、選ばれたパスを入力欄へ入れる「参照…」ボタン。 */
function makeFileBrowseButton(field: FieldDef, input: HTMLInputElement, onPicked?: () => void): HTMLButtonElement {
  return makeRowButton("参照…", null, async () => {
    try {
      const picked = await openDialog({
        directory: field.browse === "folder",
        multiple: false,
        filters: field.browse === "file" ? field.dialogFilters : undefined,
      });
      if (typeof picked === "string") {
        input.value = picked;
        onPicked?.();
      }
    } catch (e) {
      console.error("ファイル選択に失敗:", e);
    }
  });
}

/** フィールド定義が求める一覧選択ボタン。呼び出し先が渡されていないものは作らない。 */
function makePickerButtons(
  field: FieldDef,
  input: HTMLInputElement,
  pickers: Partial<FieldPickerActions>,
): HTMLButtonElement[] {
  const buttons: HTMLButtonElement[] = [];
  const add = (enabled: unknown, open: (() => void) | null, text: string, title: string) => {
    if (enabled && open) buttons.push(makeRowButton(text, title, open));
  };
  const {
    openBrowserProfilePicker, openSheetPicker, openExcelTablePicker, openCellPicker,
    openAirtableBasePicker, openAirtableTablePicker, openAirtableViewPicker,
  } = pickers;
  const { browserProfile, browseCell, browseColumn } = field;
  add(browserProfile, browserProfile && openBrowserProfilePicker ? () => openBrowserProfilePicker(browserProfile, input) : null,
    "候補から選択…", "インストール済みのブラウザからプロファイル（History/Bookmarks）を自動検出します");
  add(field.browseSheet, openSheetPicker ? () => openSheetPicker(input) : null,
    "一覧から選択…", "指定されたExcelファイルからシート名の一覧を取得して選べます");
  add(field.browseTable, openExcelTablePicker ? () => openExcelTablePicker(input) : null,
    "一覧から選択…", "指定されたExcelファイルからテーブル(ListObject)名の一覧を取得して選べます");
  add(browseCell, browseCell && openCellPicker ? () => openCellPicker(input, browseCell, "cell") : null,
    "表から選択…", "Excelシートの10x10セルを表形式で表示して選択します");
  add(browseColumn, browseColumn && openCellPicker ? () => openCellPicker(input, browseColumn, "column") : null,
    "表から選択…", "Excelシートの10x10セルを表形式で表示して基準列を選択します");
  add(field.browseAirtableBase, openAirtableBasePicker ? () => openAirtableBasePicker(input) : null,
    "一覧から選択…", "Personal Access Token を使用して利用可能な Base 一覧を取得します");
  add(field.browseAirtableTable, openAirtableTablePicker ? () => openAirtableTablePicker(input) : null,
    "一覧から選択…", "指定された Base 内のテーブル一覧を取得します");
  add(field.browseAirtableView, openAirtableViewPicker ? () => openAirtableViewPicker(input) : null,
    "一覧から選択…", "指定されたテーブル内のビュー一覧を取得します");
  return buttons;
}

/**
 * 入力欄と、その横に並べる操作（保存済みトークンの選択、参照、一覧選択、取得ページ・リンク）を
 * 1 行にまとめる。操作が 1 つも無いフィールドでは null を返し、呼び出し側は入力欄だけを置く。
 */
export function buildFieldActionRow(
  field: FieldDef,
  input: HTMLInputElement,
  options: FieldActionRowOptions,
): HTMLDivElement | null {
  const isCredOwner = !!field.credentialScope;
  const pickerButtons = makePickerButtons(field, input, options.pickers);
  if (!isCredOwner && !field.browse && !field.helpUrl && !field.linkUrl && pickerButtons.length === 0) return null;
  const row = document.createElement("div");
  row.className = "path-row";
  row.appendChild(input);
  if (isCredOwner) {
    options.wireCredentialSelector(input, field, options.idPrefix, row, options.label, options.sourceType);
  }
  if (field.browse) row.appendChild(makeFileBrowseButton(field, input, options.onFilePicked));
  row.append(...pickerButtons);
  // 秘密フィールドの取得ページ・リンクは、保存済みトークンの選択ウィジェット側が出す。
  if (!isCredOwner && field.helpUrl) row.appendChild(makeApiKeyButton(field.helpUrl));
  if (!isCredOwner && field.linkUrl) row.appendChild(makeLinkButton(field.linkUrl, field.linkLabel ?? "🌐 開く…"));
  return row;
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
  const credentialManagedKeys = credentialManagedKeysOf(fields);
  const { wireCredentialSelector, openAirtableBasePicker, openAirtableTablePicker, openAirtableViewPicker } = actions;
  const pickers = { openAirtableBasePicker, openAirtableTablePicker, openAirtableViewPicker };
  for (const field of fields) {
    const isCredManagedSibling = !field.credentialScope && credentialManagedKeys.has(field.key);
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
    const row = buildFieldActionRow(field, input, { idPrefix, sourceType, label, wireCredentialSelector, pickers });
    label.appendChild(row ?? input);
    container.appendChild(label);
  }
}
