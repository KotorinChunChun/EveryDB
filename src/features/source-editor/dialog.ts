import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { $, promptUrlInput } from "../../app/dom";
import { invoke } from "../../app/ipc";
import { appState } from "../../app/state";

import {
  parseAirtableUrl,
  parseGoogleSheetsUrl,
  parseNotionUrl,
  parseSharePointUrl,
} from "../../shared/urls";
import { TYPE_DEFS } from "../../source-types/forms";
import type { FieldDef, TypeDef } from "../../source-types/forms";
import type { ParentKind } from "../../source-types/registry";
import { SECRET_PLACEHOLDER } from "./constants";
import { isConfigIncomplete } from "./config-rules";
import { deriveDefaultName } from "./default-name";
import { parentBrowseKind } from "./connection-kinds";
import { duplicateConfirmMessage, findDuplicateSource } from "./source-identity";
import { clearLinkedCredentials, setLinkedCredentials } from "./linked-credentials";
import type { RoleEditorController } from "./role-editor";
import { configWithoutRoleKeys, roleStateFromConfig } from "./roles";
import { makeApiKeyButton, makeLinkButton } from "./ui-buttons";

export interface SourceEditorDialogElements {
  dialog: HTMLDialogElement;
  addConfigFieldset: HTMLFieldSetElement;
  configLegend: HTMLLegendElement | null;
  configLegendTitle: HTMLSpanElement;
  configAccordionBody: HTMLDivElement;
  configSummaryBadge: HTMLSpanElement;
  openTableBtn: HTMLButtonElement;
  typeSel: HTMLSelectElement;
  dynamicEl: HTMLDivElement;
  browseAreaEl: HTMLDivElement;
  belongsToEl: HTMLDivElement;
  guideBtn: HTMLButtonElement;
  nameInput: HTMLInputElement;
  addDialogTitle: HTMLDivElement;
  lastLoadedEl: HTMLDivElement;
  fetchFieldsBtn: HTMLButtonElement;
  resetFieldsBtn: HTMLButtonElement;
  addBtn: HTMLButtonElement;
  addCancelBtn: HTMLButtonElement;
  addForm: HTMLFormElement;
  addSaveStatusEl: HTMLSpanElement;
}

export interface SourceEditorDialogActions {
  parentLabel: (key: string) => string;
  parentKindOf: (type: string) => ParentKind;
  sharedKeysFor: (kind: ParentKind) => string[];
  tableOpenUrl: (type: string, cfg: Record<string, unknown>) => string | null;
  loadSources: () => Promise<void>;
  openParentSettingsDialog: (parentKey: string) => Promise<void>;
  ensureUnlocked: () => Promise<boolean>;
  guideTextFor: (type: string) => string | null;
  openGuide: (type: string) => void;
  normalizePath: (v: string | undefined | null) => string;
  roleEditor: RoleEditorController;
  pickers: {
    wireCredentialSelector: (
      input: HTMLInputElement,
      field: FieldDef,
      idPrefix: string,
      row: HTMLElement,
      label: HTMLElement,
    ) => void;
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
    openNotionPicker: () => void;
    openTablePicker: (browse: "sqlite" | "accdb" | "excel_table") => void;
    openSharePointPicker: () => void;
  };
}

export interface OpenAddDialogOptions {
  type?: string;
  title?: string;
  /** 接続先追加フロー（保存後に接続設定ダイアログを開く） */
  connectionAdd?: boolean;
}

export interface SourceEditorDialogController {
  openAddDialog: (options?: OpenAddDialogOptions) => void;
  openEditDialog: (id: number) => Promise<void>;
  getEditingId: () => number | null;
  getDialogMode: () => "add" | "edit";
  getHiddenSharedValues: () => Record<string, unknown>;
  getConfigFromFields: () => { config: Record<string, unknown>; missingText: boolean };
  maybeAutoFillName: (path?: string) => void;
  maybeFetchAndRenderFields: () => void;
  autoFillDefaultName: (force?: boolean) => void;
  applySharePointSiteSelection: (siteUrl: string, displayName: string) => Promise<void>;
  resolveAirtableCaptions: (baseId?: string, tableId?: string, viewId?: string) => Promise<void>;
}

export function mountSourceEditorDialog(
  elements: SourceEditorDialogElements,
  actions: SourceEditorDialogActions,
): SourceEditorDialogController {
  const { roleEditor } = actions;
  const { pickers } = actions;

  let dialogMode: "add" | "edit" = "add";
  let editingId: number | null = null;
  /** 編集ダイアログで接続先にグルーピングされているテーブルについて、非表示にした共有(接続)フィールドのキーと元の値。
   * buildConfigFromFields() で保存時にそのまま引き継ぐ（このダイアログでは編集させない）。 */
  let hiddenSharedKeys: string[] = [];
  let hiddenSharedValues: Record<string, unknown> = {};
  /** 追加時、表示名をユーザーが手動で編集したら以後はファイルパスからの自動入力で上書きしない。 */
  let nameManuallyEdited = false;
  /** 接続先追加フロー（保存後にテーブル選択ダイアログへ進む） */
  let connectionAddMode = false;

  for (const t of TYPE_DEFS) {
    const opt = document.createElement("option");
    opt.value = t.value;
    opt.textContent = t.label;
    elements.typeSel.appendChild(opt);
  }

  function currentTypeDef(): TypeDef {
    return TYPE_DEFS.find((t) => t.value === elements.typeSel.value)!;
  }

  /** グループボックスを折りたたみ / 展開する（デフォルトは閉じた状態） */
  function setConfigAccordionOpen(open: boolean) {
    const { addConfigFieldset, configAccordionBody, configLegendTitle } = elements;
    if (!addConfigFieldset || !configAccordionBody || !configLegendTitle) return;
    configAccordionBody.hidden = !open;
    if (open) {
      addConfigFieldset.classList.remove("collapsed");
      configLegendTitle.textContent = "▼ 接続情報・ソース設定";
    } else {
      addConfigFieldset.classList.add("collapsed");
      configLegendTitle.textContent = "▶ 接続情報・ソース設定";
    }
    updateConfigSummaryBadge();
  }

  elements.configLegend?.addEventListener("click", () => {
    if (!elements.configAccordionBody) return;
    setConfigAccordionOpen(elements.configAccordionBody.hidden);
  });

  /** 折り畳み時に現在の設定概要をバッジ表示する。 */
  function updateConfigSummaryBadge() {
    const { configAccordionBody, configSummaryBadge } = elements;
    if (!configAccordionBody || !configSummaryBadge) return;
    const def = currentTypeDef();
    if (!configAccordionBody.hidden) {
      configSummaryBadge.hidden = true;
      return;
    }
    configSummaryBadge.hidden = false;
    configSummaryBadge.textContent = `[${def ? def.label : "未選択"}]`;
  }

  /** 「このテーブルを開く」ボタンの有効/無効・表示/非表示を更新する。 */
  function updateOpenTableButton() {
    const { openTableBtn } = elements;
    if (!openTableBtn) return;
    const def = currentTypeDef();
    if (!def || dialogMode !== "edit") {
      openTableBtn.hidden = true;
      return;
    }
    const { config } = buildConfigFromFields();
    const url = actions.tableOpenUrl(def.value, config);
    if (url) {
      openTableBtn.hidden = false;
      openTableBtn.disabled = false;
      openTableBtn.onclick = async () => {
        try {
          await invoke("open_ref", { target: url });
        } catch (e) {
          alert("テーブルを開けませんでした: " + String(e));
        }
      };
    } else {
      openTableBtn.hidden = true;
    }
  }

  function getFieldValue(f: FieldDef): unknown {
    const el = document.getElementById(`fd-${f.key}`) as HTMLInputElement | null;
    if (!el) return undefined;
    // 保存済みトークンにリンク中で、まだ手入力で上書きされていなければ cred:<id> 参照を送る
    // （実際の値ではなく参照だけを保存することで、後から一括置換できる。要件: トークン管理画面）。
    if (el.dataset.credRef && el.value === SECRET_PLACEHOLDER) return `cred:${el.dataset.credRef}`;
    if (f.kind === "checkbox") return el.checked;
    if (f.kind === "list") {
      const items = el.value.split(",").map((s) => s.trim()).filter(Boolean);
      return items.length ? items : undefined;
    }
    if (f.kind === "number") {
      const v = el.value.trim();
      if (v === "") return undefined; // 空欄はサーバ側デフォルト(0)に委ねる
      const n = Number(v);
      return Number.isFinite(n) ? n : undefined;
    }
    // path 系フィールドは両端のダブルクォートを除去する（要件: クォート付きパス対応）。
    if (f.key === "path") return actions.normalizePath(el.value);
    return el.value.trim();
  }

  /** URL貼り付け等で Base/Table/View の ID が確定した直後、可能なら名前(キャプション)も
   * Airtable API から解決して base_name/table_name/view_name を自動入力する。
   * トークン未入力・未解錠・API失敗時は静かに諦める（ID自体は既に埋まっているため実害はない）。 */
  async function resolveAirtableCaptions(baseId?: string, tableId?: string, viewId?: string) {
    const tokenEl = document.getElementById("fd-token") as HTMLInputElement | null;
    const credentialId = Number(tokenEl?.dataset.credRef);
    if (!Number.isSafeInteger(credentialId) || credentialId <= 0) return;
    try {
      if (baseId) {
        const bases = await invoke("browse_airtable_bases", { credentialId });
        const b = bases.find((x) => x.id === baseId);
        const nameEl = document.getElementById("fd-base_name") as HTMLInputElement | null;
        if (b && nameEl && !nameEl.value.trim()) nameEl.value = b.name;
      }
      if (baseId && tableId) {
        const tables = await invoke("browse_airtable_tables", { credentialId, baseId });
        const t = tables.find((x) => x.id === tableId);
        const tNameEl = document.getElementById("fd-table_name") as HTMLInputElement | null;
        if (t && tNameEl && !tNameEl.value.trim()) tNameEl.value = t.name;
        if (t && viewId) {
          const v = t.views.find((x) => x.id === viewId);
          const vNameEl = document.getElementById("fd-view_name") as HTMLInputElement | null;
          if (v && vNameEl && !vNameEl.value.trim()) vNameEl.value = v.name;
        }
      }
    } catch {
      // ネットワーク不調・権限不足等。ID は既に反映済みなので実害は無い。
    } finally {
      autoFillDefaultName();
    }
  }

  /** 各種データソース URL が入力・ペーストされた際、設定 ID フィールドを自動入力・展開する。 */
  function tryApplyUrlOrFileDropHelper(inputValue: string) {
    const type = elements.typeSel.value;
    if (type === "airtable") {
      const info = parseAirtableUrl(inputValue);
      if (info) {
        if (info.baseId) {
          const baseEl = document.getElementById("fd-base_id") as HTMLInputElement | null;
          if (baseEl) baseEl.value = info.baseId;
        }
        if (info.tableId) {
          const tblEl = document.getElementById("fd-table") as HTMLInputElement | null;
          if (tblEl) tblEl.value = info.tableId;
        }
        if (info.viewId) {
          const viwEl = document.getElementById("fd-view") as HTMLInputElement | null;
          if (viwEl) viwEl.value = info.viewId;
        }
        autoFillDefaultName();
        maybeFetchAndRenderFields();
        void resolveAirtableCaptions(info.baseId, info.tableId, info.viewId);
        return;
      }
    }
    if (type === "google_sheets") {
      const info = parseGoogleSheetsUrl(inputValue);
      if (info?.spreadsheetId) {
        const el = document.getElementById("fd-spreadsheet_id") as HTMLInputElement | null;
        if (el) el.value = info.spreadsheetId;
        autoFillDefaultName();
        maybeFetchAndRenderFields();
        return;
      }
    }
    if (type === "notion") {
      const info = parseNotionUrl(inputValue);
      if (info?.databaseId) {
        const el = document.getElementById("fd-database_id") as HTMLInputElement | null;
        if (el) el.value = info.databaseId;
        autoFillDefaultName();
        maybeFetchAndRenderFields();
        return;
      }
    }
    if (type === "graph_sharepoint_list" || type === "graph_sharepoint_library") {
      const info = parseSharePointUrl(inputValue);
      if (info) {
        if (info.siteUrl) {
          const siteEl = document.getElementById("fd-site_url") as HTMLInputElement | null;
          if (siteEl) siteEl.value = info.siteUrl;
        }
        if (info.listName && type === "graph_sharepoint_list") {
          const listEl = document.getElementById("fd-list_name") as HTMLInputElement | null;
          if (listEl) listEl.value = info.listName;
        }
        if (info.libraryName && type === "graph_sharepoint_library") {
          const libEl = document.getElementById("fd-library_name") as HTMLInputElement | null;
          if (libEl) libEl.value = info.libraryName;
        }
        autoFillDefaultName();
        maybeFetchAndRenderFields();
        return;
      }
    }
  }

  /** 追加ダイアログ等でデータソース設定・種別の変更時や未入力時に表示名を自動入力・補完する。 */
  function autoFillDefaultName(force: boolean = false) {
    if (dialogMode !== "add") return;
    if (!force && nameManuallyEdited) return;
    const { config } = buildConfigFromFields();
    const name = deriveDefaultName(elements.typeSel.value, config);
    if (name) {
      elements.nameInput.value = name;
    }
  }

  /** 追加ダイアログでファイルパスを入力/選択した際、表示名が未編集ならファイル名を自動入力する（互換関数）。 */
  function maybeAutoFillName(_path?: string) {
    autoFillDefaultName();
  }

  /** 接続設定（ファイルパスやテーブル名など必須項目）がすべて入力・選択され確定した際に、自動で後半の列一覧表を再読込する。 */
  function maybeFetchAndRenderFields() {
    if (dialogMode !== "add" && dialogMode !== "edit") return;
    const { missingText } = buildConfigFromFields();
    if (!missingText) {
      void fetchAndRenderFields();
    } else {
      roleEditor.reset();
    }
  }

  /** excel_range の start_mode/end_mode の選択に連動して start_cell/start_ref_column 及び end_cell/end_ref_column の表示/非表示を切り替える。 */
  function syncExcelRangeModeVisibility() {
    if (elements.typeSel.value !== "excel_range") return;
    const toggleField = (modeKey: string, cellKey: string, refColKey: string) => {
      const modeSel = document.getElementById(`fd-${modeKey}`) as HTMLSelectElement | null;
      const cellLabel = document.getElementById(`fd-${cellKey}`)?.closest("label") as HTMLElement | null;
      const refColLabel = document.getElementById(`fd-${refColKey}`)?.closest("label") as HTMLElement | null;
      if (!modeSel) return;
      const manual = modeSel.value === "manual";
      if (cellLabel) cellLabel.style.display = manual ? "" : "none";
      if (refColLabel) refColLabel.style.display = manual ? "none" : "";
    };
    toggleField("start_mode", "start_cell", "start_ref_column");
    toggleField("end_mode", "end_cell", "end_ref_column");
  }

  function createBrowseButton(def: TypeDef): HTMLButtonElement | null {
    if (!def.browse) return null;
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "btn";
    switch (def.browse) {
      case "notion":
        btn.textContent = "データベース一覧から選択…";
        btn.onclick = pickers.openNotionPicker;
        break;
      case "sqlite":
        btn.textContent = "テーブル一覧から複数追加…";
        btn.onclick = () => pickers.openTablePicker("sqlite");
        break;
      case "accdb":
        btn.textContent = "テーブル一覧から複数追加…";
        btn.onclick = () => pickers.openTablePicker("accdb");
        break;
      case "excel_table":
        btn.textContent = "テーブル(ListObject)一覧から複数追加…";
        btn.onclick = () => pickers.openTablePicker("excel_table");
        break;
      case "sharepoint":
        btn.textContent = "サイト・チームから選択…";
        btn.onclick = pickers.openSharePointPicker;
        break;
      case "airtable":
        btn.textContent = "Base 一覧から選択…";
        btn.onclick = () => {
          const baseEl = document.getElementById("fd-base_id") as HTMLInputElement | null;
          if (baseEl) pickers.openAirtableBasePicker(baseEl);
        };
        break;
    }
    return btn;
  }

  function renderAutoSetupRow(def: TypeDef) {
    const field = document.createElement("div");
    field.className = "field auto-setup-row";
    const labelSpan = document.createElement("span");
    labelSpan.textContent = "自動設定";
    field.appendChild(labelSpan);

    const btnRow = document.createElement("div");
    btnRow.className = "zoom-row";
    const urlBtn = document.createElement("button");
    urlBtn.type = "button";
    urlBtn.className = "btn";
    urlBtn.textContent = "URLから自動入力";
    urlBtn.onclick = async () => {
      const url = await promptUrlInput();
      if (url) tryApplyUrlOrFileDropHelper(url);
    };
    btnRow.appendChild(urlBtn);
    if (dialogMode === "add" && def.browse === "sharepoint") {
      const browseBtn = createBrowseButton(def);
      if (browseBtn) btnRow.appendChild(browseBtn);
    }
    field.appendChild(btnRow);
    elements.dynamicEl.appendChild(field);
  }

  function renderBrowseArea(def: TypeDef) {
    elements.browseAreaEl.innerHTML = "";
    if (!def.browse || def.browse === "sharepoint") return;
    const btn = createBrowseButton(def);
    if (btn) elements.browseAreaEl.appendChild(btn);
  }

  async function applySharePointSiteSelection(siteUrl: string, displayName: string) {
    const siteEl = document.getElementById("fd-site_url") as HTMLInputElement | null;
    if (siteEl) siteEl.value = siteUrl;
    const siteNameEl = document.getElementById("fd-site_display_name") as HTMLInputElement | null;
    if (siteNameEl) siteNameEl.value = displayName;
    const listEl = document.getElementById("fd-list_name") as HTMLInputElement | null;
    if (listEl) listEl.value = "";
    const libEl = document.getElementById("fd-library_name") as HTMLInputElement | null;
    if (libEl) libEl.value = "";
    if (displayName) {
      try {
        await invoke("set_parent_label", { parentKey: siteUrl, label: displayName });
        appState.parentLabelOverrides.set(siteUrl, displayName);
      } catch (e) {
        console.warn("接続先ラベルの保存に失敗:", e);
      }
    }
    autoFillDefaultName();
    maybeFetchAndRenderFields();
  }

  function renderTypeFields(prefill?: Record<string, unknown>, hideKeys: string[] = []) {
    const def = currentTypeDef();
    elements.dynamicEl.innerHTML = "";
    let currentInlineRow: HTMLElement | null = null;
    // 秘密フィールドの束（token/client_secret/password 本体 + tenant_id 等のバンドル）を先に洗い出す。
    // 束の非秘密フィールドは選択専用ウィジェットが埋めるため、個別入力は読み取り専用にする
    // （要件: 接続単位での保存は不要、全て管理されているトークンを参照させる）。
    const credentialManagedKeys = new Set<string>();
    for (const f of def.fields) {
      if (f.credentialScope) {
        credentialManagedKeys.add(f.key);
        for (const bf of f.credentialBundleFields ?? []) credentialManagedKeys.add(bf);
      }
    }
    for (const f of def.fields) {
      if (hideKeys.includes(f.key)) continue;
      if (f.key === "site_display_name") {
        const hidden = document.createElement("input");
        hidden.type = "text";
        hidden.id = `fd-${f.key}`;
        hidden.hidden = true;
        if (prefill && prefill[f.key] !== undefined && prefill[f.key] !== null) {
          hidden.value = String(prefill[f.key]);
        }
        elements.dynamicEl.appendChild(hidden);
        continue;
      }
      if (f.key === "url_helper") {
        renderAutoSetupRow(def);
        continue;
      }
      const isCredOwner = !!f.credentialScope;
      const isCredManagedSibling = !isCredOwner && credentialManagedKeys.has(f.key);
      const label = document.createElement("label");
      if (f.kind === "checkbox") {
        label.className = "check";
        const cb = document.createElement("input");
        cb.type = "checkbox";
        cb.id = `fd-${f.key}`;
        cb.checked = prefill ? Boolean(prefill[f.key]) : (f.default ?? false);
        cb.addEventListener("change", () => {
          autoFillDefaultName();
          maybeFetchAndRenderFields();
        });
        label.append(cb, document.createTextNode(" " + f.label));
      } else if (f.kind === "select") {
        label.textContent = f.label;
        const sel = document.createElement("select");
        sel.id = `fd-${f.key}`;
        for (const opt of f.options ?? []) {
          const o = document.createElement("option");
          o.value = opt.value;
          o.textContent = opt.label;
          sel.appendChild(o);
        }
        if (prefill && typeof prefill[f.key] === "string") sel.value = prefill[f.key] as string;
        sel.addEventListener("change", () => {
          autoFillDefaultName();
          maybeFetchAndRenderFields();
          // excel_range の start_mode/end_mode 連動表示
          syncExcelRangeModeVisibility();
        });
        label.appendChild(sel);
      } else if (f.kind === "textarea") {
        label.textContent = f.label;
        const ta = document.createElement("textarea");
        ta.id = `fd-${f.key}`;
        ta.rows = 4;
        ta.className = "web-page-columns";
        if (f.placeholder) ta.placeholder = f.placeholder;
        if (prefill && typeof prefill[f.key] === "string") ta.value = prefill[f.key] as string;
        ta.addEventListener("input", () => autoFillDefaultName());
        ta.addEventListener("change", () => maybeFetchAndRenderFields());
        label.appendChild(ta);
      } else {
        label.textContent = f.label;
        const input = document.createElement("input");
        input.type = f.kind === "password" ? "password" : f.kind === "number" ? "number" : "text";
        input.autocomplete = f.kind === "password" ? "off" : "on";
        input.id = `fd-${f.key}`;
        if (f.kind === "number" && f.min !== undefined) input.min = String(f.min);
        if (f.placeholder && !isCredManagedSibling) input.placeholder = f.placeholder;
        if (prefill) {
          const v = prefill[f.key];
          if (Array.isArray(v)) input.value = v.join(", ");
          else if (v !== undefined && v !== null) input.value = String(v);
        }
        if (isCredManagedSibling) {
          // トークンのバンドルフィールド（tenant_id 等）は選択専用ウィジェットが埋めるだけの
          // データキャリアで、画面には一切出さない（要件: 接続画面はプロファイル名だけ見せる）。
          input.hidden = true;
          elements.dynamicEl.appendChild(input);
          continue;
        }
        input.addEventListener("input", () => autoFillDefaultName());
        input.addEventListener("change", () => {
          autoFillDefaultName();
          maybeFetchAndRenderFields();
          // URL貼り付けが ID 欄より先にトークンより先行した場合など、トークン確定時点で
          // まだキャプション（base_name等）が空の ID があれば遅れて解決する。
          if (f.key === "token" && def.value === "airtable") {
            const baseId = (document.getElementById("fd-base_id") as HTMLInputElement | null)?.value.trim();
            const tableId = (document.getElementById("fd-table") as HTMLInputElement | null)?.value.trim();
            const viewId = (document.getElementById("fd-view") as HTMLInputElement | null)?.value.trim();
            if (baseId || tableId) void resolveAirtableCaptions(baseId || undefined, tableId || undefined, viewId || undefined);
          }
        });
        if (isCredOwner || f.browse || f.helpUrl || f.linkUrl || f.browserProfile || f.browseSheet || f.browseTable || f.browseCell || f.browseColumn || f.browseAirtableBase || f.browseAirtableTable || f.browseAirtableView) {
          // 参照ボタン（ファイル/フォルダ選択）、API キー取得ページを開くボタン、リンクボタン、
          // ブラウザプロファイル候補自動検出、Excelシート名選択、Excelテーブル(ListObject)名選択、Excelセル/列選択、
          // Airtable選択ボタン、保存済みトークンの選択ウィジェット。
          const row = document.createElement("div");
          row.className = "path-row";
          row.appendChild(input);
          if (isCredOwner) pickers.wireCredentialSelector(input, f, "fd-", row, label);
          if (f.browse) {
            const browseBtn = document.createElement("button");
            browseBtn.type = "button";
            browseBtn.className = "btn browse-btn";
            browseBtn.textContent = "参照…";
            browseBtn.onclick = async () => {
              try {
                const picked = await openDialog({
                  directory: f.browse === "folder",
                  multiple: false,
                  filters: f.browse === "file" ? f.dialogFilters : undefined,
                });
                if (typeof picked === "string") {
                  input.value = picked;
                  autoFillDefaultName();
                  maybeFetchAndRenderFields();
                }
              } catch (e) {
                console.error("ファイル選択に失敗:", e);
              }
            };
            row.appendChild(browseBtn);
          }
          if (f.browserProfile) {
            const pickBtn = document.createElement("button");
            pickBtn.type = "button";
            pickBtn.className = "btn browse-btn";
            pickBtn.textContent = "候補から選択…";
            pickBtn.title = "インストール済みのブラウザからプロファイル（History/Bookmarks）を自動検出します";
            pickBtn.onclick = () => pickers.openBrowserProfilePicker(f.browserProfile!, input);
            row.appendChild(pickBtn);
          }
          if (f.browseSheet) {
            const pickBtn = document.createElement("button");
            pickBtn.type = "button";
            pickBtn.className = "btn browse-btn";
            pickBtn.textContent = "一覧から選択…";
            pickBtn.title = "指定されたExcelファイルからシート名の一覧を取得して選べます";
            pickBtn.onclick = () => pickers.openSheetPicker(input);
            row.appendChild(pickBtn);
          }
          if (f.browseTable) {
            const pickBtn = document.createElement("button");
            pickBtn.type = "button";
            pickBtn.className = "btn browse-btn";
            pickBtn.textContent = "一覧から選択…";
            pickBtn.title = "指定されたExcelファイルからテーブル(ListObject)名の一覧を取得して選べます";
            pickBtn.onclick = () => pickers.openExcelTablePicker(input);
            row.appendChild(pickBtn);
          }
          if (f.browseCell) {
            const pickBtn = document.createElement("button");
            pickBtn.type = "button";
            pickBtn.className = "btn browse-btn";
            pickBtn.textContent = "表から選択…";
            pickBtn.title = "Excelシートの10x10セルを表形式で表示して選択します";
            pickBtn.onclick = () => pickers.openCellPicker(input, f.browseCell!, "cell");
            row.appendChild(pickBtn);
          }
          if (f.browseColumn) {
            const pickBtn = document.createElement("button");
            pickBtn.type = "button";
            pickBtn.className = "btn browse-btn";
            pickBtn.textContent = "表から選択…";
            pickBtn.title = "Excelシートの10x10セルを表形式で表示して基準列を選択します";
            pickBtn.onclick = () => pickers.openCellPicker(input, f.browseColumn!, "column");
            row.appendChild(pickBtn);
          }
          if (f.browseAirtableBase) {
            const pickBtn = document.createElement("button");
            pickBtn.type = "button";
            pickBtn.className = "btn browse-btn";
            pickBtn.textContent = "一覧から選択…";
            pickBtn.title = "Personal Access Token を使用して利用可能な Base 一覧を取得します";
            pickBtn.onclick = () => pickers.openAirtableBasePicker(input);
            row.appendChild(pickBtn);
          }
          if (f.browseAirtableTable) {
            const pickBtn = document.createElement("button");
            pickBtn.type = "button";
            pickBtn.className = "btn browse-btn";
            pickBtn.textContent = "一覧から選択…";
            pickBtn.title = "指定された Base 内のテーブル一覧を取得します";
            pickBtn.onclick = () => pickers.openAirtableTablePicker(input);
            row.appendChild(pickBtn);
          }
          if (f.browseAirtableView) {
            const pickBtn = document.createElement("button");
            pickBtn.type = "button";
            pickBtn.className = "btn browse-btn";
            pickBtn.textContent = "一覧から選択…";
            pickBtn.title = "指定されたテーブル内のビュー一覧を取得します";
            pickBtn.onclick = () => pickers.openAirtableViewPicker(input);
            row.appendChild(pickBtn);
          }
          if (!isCredOwner && f.helpUrl) row.appendChild(makeApiKeyButton(f.helpUrl));
          if (!isCredOwner && f.linkUrl) row.appendChild(makeLinkButton(f.linkUrl, f.linkLabel ?? "🌐 開く…"));
          label.appendChild(row);
        } else {
          label.appendChild(input);
        }
      }
      if (elements.typeSel.value === "excel_range") {
        if (f.key === "start_mode" || f.key === "end_mode") {
          currentInlineRow = document.createElement("div");
          currentInlineRow.className = "range-inline-row";
          elements.dynamicEl.appendChild(currentInlineRow);
        }
        if (currentInlineRow && (f.key === "start_mode" || f.key === "start_cell" || f.key === "start_ref_column" || f.key === "end_mode" || f.key === "end_cell" || f.key === "end_ref_column")) {
          label.style.margin = "0";
          label.style.flex = "1 1 auto";
          currentInlineRow.appendChild(label);
          continue;
        } else {
          currentInlineRow = null;
        }
      }
      elements.dynamicEl.appendChild(label);
    }
    // 参照(ブラウズ)ボタンは新規追加時のみ（編集時は種別固定・接続情報も隠れているため意味がない）。
    if (dialogMode === "add") {
      renderBrowseArea(def);
    } else {
      elements.browseAreaEl.innerHTML = "";
    }
    elements.guideBtn.hidden = actions.guideTextFor(elements.typeSel.value) === null;
    roleEditor.reset();
    updateConfigSummaryBadge();
    updateOpenTableButton();
    if (dialogMode === "add") {
      autoFillDefaultName();
    }
    // excel_range の start_mode/end_mode 連動表示（初期状態）
    syncExcelRangeModeVisibility();
  }

  elements.typeSel.onchange = () => {
    renderTypeFields();
    autoFillDefaultName();
    maybeFetchAndRenderFields();
  };

  function buildConfigFromFields(): { config: Record<string, unknown>; missingText: boolean } {
    const def = currentTypeDef();
    // 接続先側で管理している共有(接続)フィールドは、このダイアログでは表示していないので元の値をそのまま引き継ぐ。
    const config: Record<string, unknown> = { ...hiddenSharedValues };
    for (const f of def.fields) {
      if (hiddenSharedKeys.includes(f.key)) continue;
      const v = getFieldValue(f);
      if (v === undefined) continue;
      // 空の任意テキスト/パスワードは送らない（サーバ側の Option<...>/デフォルトに委ねる）。
      if (f.optional && (f.kind === "text" || f.kind === "password") && String(v).trim() === "") continue;
      config[f.key] = v;
    }
    return { config, missingText: isConfigIncomplete(elements.typeSel.value, def.fields, config) };
  }

  /** フィールド一覧を取得してロールテーブルを描画する。編集ダイアログでは開いた直後に自動実行し、
   * ボタン自体は変更後の内容を取り直す「再読込」として機能する。 */
  async function fetchAndRenderFields() {
    roleEditor.setHint("取得中…");
    try {
      let names: string[];
      let fromCache = false;
      if (dialogMode === "edit" && editingId !== null) {
        const preview = await invoke("preview_source_fields_by_id", { id: editingId });
        names = preview.names;
        fromCache = preview.from_cache;
      } else {
        const { config } = buildConfigFromFields();
        names = await invoke("preview_source_fields", { type: elements.typeSel.value, configJson: JSON.stringify(config) });
      }
      roleEditor.render(names);
      if (fromCache) {
        roleEditor.appendHint("（⚠ 接続できないためキャッシュ済みデータから復元。表示名ベースの列名です）");
        setConfigAccordionOpen(true);
      }
    } catch (e) {
      roleEditor.setHint("取得失敗: " + String(e));
      setConfigAccordionOpen(true);
    }
  }

  elements.fetchFieldsBtn.onclick = () => void fetchAndRenderFields();

  /** 表示・検索設定(役割/エイリアス/並び順)をこのダイアログ上で既定値へ戻す（保存はまだされない）。 */
  elements.resetFieldsBtn.onclick = () => {
    roleEditor.setRoleState(null);
    roleEditor.render([...roleEditor.getLastFieldNames()]);
  };

  function openAddDialog(options: OpenAddDialogOptions = {}) {
    dialogMode = "add";
    editingId = null;
    roleEditor.setRoleState(null);
    clearLinkedCredentials();
    hiddenSharedKeys = [];
    hiddenSharedValues = {};
    nameManuallyEdited = false;
    connectionAddMode = options.connectionAdd ?? false;
    elements.belongsToEl.hidden = true;
    elements.lastLoadedEl.hidden = true;
    elements.addDialogTitle.textContent = options.title ?? (connectionAddMode ? "接続先を追加" : "テーブル追加");
    elements.nameInput.value = "";
    elements.typeSel.value = options.type ?? "csv";
    elements.typeSel.disabled = false;
    setConfigAccordionOpen(true);
    renderTypeFields();
    elements.dialog.showModal();
  }

  elements.nameInput.addEventListener("input", () => {
    nameManuallyEdited = true;
    updateConfigSummaryBadge();
  });

  async function openEditDialog(id: number) {
    const detail = await invoke("get_source_detail", { id });
    const info = appState.sources.find((s) => s.id === id);
    dialogMode = "edit";
    editingId = id;
    setLinkedCredentials(detail.linked_credentials ?? {});
    elements.addDialogTitle.textContent = `編集: ${detail.name}`;
    if (info?.last_success_at) {
      elements.lastLoadedEl.hidden = false;
      elements.lastLoadedEl.textContent = `最終読み込み: ${info.last_success_at}` + (info.status === "error" ? "（直近は失敗: 前回成功時点のキャッシュを表示中）" : "");
    } else {
      elements.lastLoadedEl.hidden = false;
      elements.lastLoadedEl.textContent = "最終読み込み: まだ取得されていません";
    }
    elements.nameInput.value = detail.name;
    elements.typeSel.value = detail.type_;
    elements.typeSel.disabled = true; // 種別変更は複製で対応(接続情報の構造が変わるため編集では不可)

    // フィールド役割情報を config から抽出し、prefill から役割系キーは除いて渡す。
    const cfg = { ...detail.config };
    roleEditor.setRoleState(roleStateFromConfig(cfg));
    const prefill = configWithoutRoleKeys(cfg);

    // 接続先にグルーピングされているテーブルは、共有の接続情報フィールドを
    // このダイアログから隠す（変更は接続先の右クリックメニューから行う）。保存時は元の値を引き継ぐ。
    const parentKey = info?.parent ?? "";
    hiddenSharedKeys = parentKey ? actions.sharedKeysFor(actions.parentKindOf(detail.type_)) : [];
    hiddenSharedValues = {};
    for (const k of hiddenSharedKeys) {
      hiddenSharedValues[k] = prefill[k] ?? (k === "path" ? parentKey : undefined);
    }
    if (parentKey && hiddenSharedKeys.length > 0) {
      elements.belongsToEl.hidden = false;
      elements.belongsToEl.textContent = `所属: ${actions.parentLabel(parentKey)}（接続情報は接続先を右クリック→接続設定から変更できます）`;
    } else {
      elements.belongsToEl.hidden = true;
    }

    const hasError = info?.status === "error" || !info?.last_success_at;
    setConfigAccordionOpen(hasError);
    renderTypeFields(prefill, hiddenSharedKeys);
    elements.dialog.showModal();

    // 編集ダイアログでは開いた直後にフィールド一覧を自動取得して表示する。
    // 既存の「再読込」ボタンは、接続情報を変更した後などに手動で取り直す用途になる。
    await fetchAndRenderFields();
    if (info?.status === "error" || !info?.last_success_at) {
      setConfigAccordionOpen(true);
    }
    if (detail.has_secret && !detail.unlocked) {
      roleEditor.appendHint("（シークレットはロック中: 保存すると変更なし扱いになります）");
    }
  }

  elements.addCancelBtn.onclick = () => elements.dialog.close("cancel");

  elements.addForm.addEventListener("submit", async (e) => {
    const submitter = (e as SubmitEvent).submitter as HTMLButtonElement | null;
    if (submitter && submitter.value === "cancel") {
      elements.dialog.close("cancel");
      return;
    }
    // method="dialog" のフォームは、既定動作としてこの submit ハンドラの同期部分が終わった時点で
    // ダイアログを閉じてしまう。初回取得（reload_one）が終わるまではダイアログを開いたままにして
    // 読み込み中であることを示すため、既定動作を止める（要件: 保存時のフリーズ感の解消）。
    e.preventDefault();
    let name = elements.nameInput.value.trim();
    const { config, missingText } = buildConfigFromFields();
    if (!name) {
      name = deriveDefaultName(elements.typeSel.value, config);
      if (name) elements.nameInput.value = name;
    }
    if (!name || missingText) return;
    Object.assign(config, roleEditor.collectConfig());
    const okBtn = submitter ?? elements.addForm.querySelector<HTMLButtonElement>('button[value="ok"]')!;
    const cancelBtn = elements.addCancelBtn;
    const statusEl = elements.addSaveStatusEl;
    okBtn.disabled = true;
    cancelBtn.disabled = true;
    statusEl.hidden = false;
    try {
      if (dialogMode === "edit" && editingId !== null) {
        await invoke("update_source", { id: editingId, name, configJson: JSON.stringify(config) });
        elements.typeSel.disabled = false;
        await actions.loadSources();
        elements.dialog.close();
      } else {
        const type_ = elements.typeSel.value;
        const dup = await findDuplicateSource(type_, config, appState.sources);
        if (dup) {
          if (window.confirm(duplicateConfirmMessage(dup.name))) {
            await invoke("reload_sources", { sourceIds: [dup.id] });
            await actions.loadSources();
            elements.dialog.close();
          }
          return;
        }
        const newId = await invoke("add_source", { type: type_, name, configJson: JSON.stringify(config) });
        elements.typeSel.disabled = false;
        await actions.loadSources();
        elements.dialog.close();
        const added = appState.sources.find((s) => s.id === newId);
        if (connectionAddMode && added && parentBrowseKind(type_) !== null) {
          await actions.openParentSettingsDialog(added.parent);
        }
      }
    } catch (err) {
      alert("保存に失敗: " + String(err));
    } finally {
      okBtn.disabled = false;
      cancelBtn.disabled = false;
      statusEl.hidden = true;
    }
  });

  elements.dialog.addEventListener("dragover", (ev) => {
    ev.preventDefault();
    ev.stopPropagation();
  });
  elements.dialog.addEventListener("drop", (ev) => {
    ev.preventDefault();
    ev.stopPropagation();
    const files = ev.dataTransfer?.files;
    if (files && files.length > 0) {
      const file = files[0];
      const rawPath = (file as File & { path?: string }).path || file.name;
      const path = actions.normalizePath(rawPath);
      const pathEl = document.getElementById("fd-path") as HTMLInputElement | null;
      const keyPathEl = document.getElementById("fd-key_path") as HTMLInputElement | null;
      if (pathEl) {
        pathEl.value = path;
        autoFillDefaultName();
        maybeFetchAndRenderFields();
      } else if (keyPathEl) {
        keyPathEl.value = path;
        autoFillDefaultName();
        maybeFetchAndRenderFields();
      } else {
        tryApplyUrlOrFileDropHelper(path);
      }
    }
  });

  // ---- 秘密フィールドは「表示」時に Windows Hello の本人確認を要求する ----
  // パスワード種別の入力(password type)はブラウザ標準でマスクされるため追加対応不要。
  // ロック中の編集ダイアログでプレースホルダーが入っている場合、フィールドをクリックしたら解錠を促す。
  elements.dynamicEl.addEventListener("focusin", async (ev) => {
    const el = ev.target as HTMLInputElement;
    if (el.type !== "password") return;
    if (el.value !== SECRET_PLACEHOLDER) return;
    // 保存済みトークンへのリンク中は実値が無い（credentials 側にある）ため解錠を促す意味が無い。
    if (el.dataset.credLabel) return;
    const ok = await actions.ensureUnlocked();
    if (ok && editingId !== null) {
      const detail = await invoke("get_source_detail", { id: editingId });
      el.value = String((detail.config as Record<string, unknown>)[el.id.replace("fd-", "")] ?? "");
    }
  });

  elements.guideBtn.onclick = () => actions.openGuide(elements.typeSel.value);

  return {
    openAddDialog,
    openEditDialog,
    getEditingId: () => editingId,
    getDialogMode: () => dialogMode,
    getHiddenSharedValues: () => hiddenSharedValues,
    getConfigFromFields: buildConfigFromFields,
    maybeAutoFillName,
    maybeFetchAndRenderFields,
    autoFillDefaultName,
    applySharePointSiteSelection,
    resolveAirtableCaptions,
  };
}

export function mountSourceEditorDialogFromDom(
  actions: SourceEditorDialogActions,
): SourceEditorDialogController {
  return mountSourceEditorDialog({
    dialog: $<HTMLDialogElement>("add-dialog"),
    addConfigFieldset: $<HTMLFieldSetElement>("add-config-fieldset"),
    configLegend: document.getElementById("config-legend") as HTMLLegendElement | null,
    configLegendTitle: $<HTMLSpanElement>("config-legend-title"),
    configAccordionBody: $<HTMLDivElement>("config-accordion-body"),
    configSummaryBadge: $<HTMLSpanElement>("config-summary-badge"),
    openTableBtn: $<HTMLButtonElement>("f-open-table"),
    typeSel: $<HTMLSelectElement>("f-type"),
    dynamicEl: $<HTMLDivElement>("f-dynamic"),
    browseAreaEl: $<HTMLDivElement>("browse-area"),
    belongsToEl: $<HTMLDivElement>("f-belongs-to"),
    guideBtn: $<HTMLButtonElement>("f-guide"),
    nameInput: $<HTMLInputElement>("f-name"),
    addDialogTitle: $<HTMLDivElement>("add-dialog-title"),
    lastLoadedEl: $<HTMLDivElement>("f-last-loaded"),
    fetchFieldsBtn: $<HTMLButtonElement>("fetch-fields"),
    resetFieldsBtn: $<HTMLButtonElement>("reset-fields"),
    addBtn: $<HTMLButtonElement>("add"),
    addCancelBtn: $<HTMLButtonElement>("add-cancel"),
    addForm: $<HTMLFormElement>("add-form"),
    addSaveStatusEl: $<HTMLSpanElement>("add-save-status"),
  }, actions);
}
