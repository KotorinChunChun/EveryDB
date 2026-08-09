import { $, confirmDelete, setPickerMessage } from "../../app/dom";
import { invoke } from "../../app/ipc";
import { appState } from "../../app/state";
import type { SourceInfo } from "../../shared/types";
import { TYPE_DEFS } from "../../source-types/forms";
import type { FieldDef } from "../../source-types/forms";
import type { ParentKind } from "../../source-types/registry";
import { sourceTypeLabel } from "../sources/tree";
import { SECRET_PLACEHOLDER } from "./constants";
import type { FieldRendererActions } from "./field-renderer";
import { renderFieldsInto } from "./field-renderer";
import { setLinkedCredentials } from "./linked-credentials";
import { GRAPH_ONE_CLICK_TYPES, parentBrowseKind, type ParentBrowseKind } from "./connection-kinds";
import { duplicateConfirmMessage, findDuplicateSource } from "./source-identity";
import { sharedConfigWouldChange } from "./shared-values-diff";
import { currentFieldRawValue, parentKindLabel, sharedKeysFor } from "./shared-config";
/** Microsoft 365: 対象ユーザーやグループ ID など追加入力が要るため一覧からは追加できない種別。 */
const GRAPH_PARAMETERIZED_TYPES = ["graph_calendar", "graph_onedrive", "graph_planner"];

/** 接続先種別ごとの「アクセス許可（共有）設定ページ」。テーブル一覧が 0 件のときの導線。 */
const ACCESS_SETTINGS_URL: Partial<Record<ParentBrowseKind, { url: string; label: string; hint: string }>> = {
  airtable: {
    url: "https://airtable.com/workspaces",
    label: "ワークスペースを開く…",
    hint: "実在する Base や Table 一覧を取得するには、Personal Access Token に「schema.bases:read」および「data.records:read」スコープが必要です。",
  },
  notion: {
    url: "https://www.notion.so/my-integrations",
    label: "アクセス許可を設定…",
    hint: "一覧に出るのは、Notion 側でこのインテグレーションに「接続」したデータベースだけです。"
      + "対象のデータベースを開き、右上の … →「接続」からインテグレーションを追加してから、⟳ 更新を押してください。",
  },
  graph365: {
    url: "https://entra.microsoft.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade",
    label: "API アクセス許可を設定…",
    hint: "取得には、アプリ登録の「API のアクセス許可」で対象の Graph 権限（User.Read.All 等）に"
      + "管理者の同意が必要です。",
  },
  sharepoint: {
    url: "https://entra.microsoft.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade",
    label: "API アクセス許可を設定…",
    hint: "サイト内のリストとドキュメントライブラリを一覧します。取得できない場合はアプリ登録の"
      + "「API のアクセス許可」（Sites.Read.All 等）を確認してください。",
  },
};

interface ParentDialogState {
  parentKey: string;
  keys: string[];
  fieldDefs: FieldDef[];
  children: SourceInfo[];
  repType: string;
}

interface ParentTableItem { key: string; label: string; type_: string; extraConfig: Record<string, unknown> }

export interface ParentSettingsDialogElements {
  dialog: HTMLDialogElement;
  titleEl: HTMLDivElement;
  typeLabelEl: HTMLDivElement;
  guideBtn: HTMLButtonElement;
  labelInput: HTMLInputElement;
  scopeHintEl: HTMLDivElement;
  dynamicEl: HTMLDivElement;
  errorEl: HTMLDivElement;
  cancelBtn: HTMLButtonElement;
  closeXBtn: HTMLButtonElement;
  saveBtn: HTMLButtonElement;
  saveStatusEl: HTMLSpanElement;
  tablesFieldset: HTMLFieldSetElement;
  tablesList: HTMLDivElement;
  tablesHint: HTMLDivElement;
  tablesCount: HTMLSpanElement;
  tablesAccessBtn: HTMLButtonElement;
  tablesAllBtn: HTMLButtonElement;
  tablesNoneBtn: HTMLButtonElement;
  tablesRefreshBtn: HTMLButtonElement;
}

export interface ParentSettingsDialogActions {
  parentLabel: (parentKey: string) => string;
  parentKindOf: (type_: string) => ParentKind;
  guideTextFor: (type_: string) => string | null;
  openGuide: (type_: string) => void;
  ensureUnlocked: () => Promise<boolean>;
  loadSources: () => Promise<void>;
  runSearch: () => Promise<void>;
  normalizePath: (value: string | undefined | null) => string;
  fieldRenderer: FieldRendererActions;
}

export interface ParentSettingsDialogController {
  openParentSettingsDialog: (parentKey: string) => Promise<void>;
  getRepType: () => string | null;
  getSiblingSourceId: () => number | null;
  getParentSharedValues: () => Record<string, string>;
}

export function mountParentSettingsDialog(
  elements: ParentSettingsDialogElements,
  actions: ParentSettingsDialogActions,
): ParentSettingsDialogController {
  let parentDialogState: ParentDialogState | null = null;
  let parentTableMatched = new Map<string, number>();
  let parentTableItems: ParentTableItem[] = [];

  function fieldRawValue(el: HTMLInputElement, key: string): string {
    return currentFieldRawValue(el, key, actions.normalizePath);
  }

  function parentSharedNow(): Record<string, string> {
    const out: Record<string, string> = {};
    if (!parentDialogState) return out;
    for (const k of parentDialogState.keys) {
      const el = document.getElementById(`pf-${k}`) as HTMLInputElement | null;
      if (el) out[k] = fieldRawValue(el, k);
    }
    return out;
  }

  function updateParentTablesCount() {
    const boxes = Array.from(elements.tablesList.querySelectorAll<HTMLInputElement>("input[type=checkbox]"));
    const checked = boxes.filter((cb) => cb.checked).length;
    elements.tablesCount.textContent = boxes.length === 0 ? "" : `${checked} / ${boxes.length} 件を取り込み`;
  }

  function parentTableDiff(): { toAdd: ParentTableItem[]; toRemove: { id: number; label: string }[] } {
    const toAdd: ParentTableItem[] = [];
    const toRemove: { id: number; label: string }[] = [];
    if (elements.tablesFieldset.hidden) return { toAdd, toRemove };
    for (const cb of Array.from(elements.tablesList.querySelectorAll<HTMLInputElement>("input[type=checkbox]"))) {
      const item = parentTableItems.find((i) => i.key === cb.dataset.key);
      if (!item) continue;
      const existingId = parentTableMatched.get(item.key);
      if (cb.checked && existingId === undefined) toAdd.push(item);
      else if (!cb.checked && existingId !== undefined) toRemove.push({ id: existingId, label: item.label });
    }
    return { toAdd, toRemove };
  }

  /** 接続先で使えるテーブルの一覧を取得して描画する。 */
  async function loadParentTables() {
    if (!parentDialogState) return;
    const { children, repType } = parentDialogState;
    const kind = parentBrowseKind(repType);
    elements.tablesFieldset.hidden = kind === null;
    elements.tablesAccessBtn.hidden = true;
    elements.tablesCount.textContent = "";
    if (!kind) return;

    const access = ACCESS_SETTINGS_URL[kind];
    if (access) {
      elements.tablesAccessBtn.hidden = false;
      elements.tablesAccessBtn.textContent = access.label;
      elements.tablesAccessBtn.title = `${access.url} を既定のブラウザで開きます`;
      elements.tablesAccessBtn.onclick = () => void invoke("open_ref", { target: access.url }).catch((e) => {
        elements.tablesHint.textContent = "ページを開けませんでした: " + String(e);
      });
    }
    elements.tablesHint.textContent = access?.hint ?? "";

    elements.tablesList.innerHTML = "<div class='picker-empty'>読み込み中…</div>";
    const shared = parentSharedNow();
    const siblingId = children[0].id;

    parentTableItems = [];
    try {
      if (kind === "sqlite" || kind === "accdb" || kind === "excel_table") {
        const cmd = kind === "sqlite" ? "browse_sqlite_tables" : kind === "accdb" ? "browse_accdb_tables" : "browse_excel_tables";
        const names = await invoke(cmd, { path: shared.path });
        parentTableItems = names.map((n) => ({ key: n, label: n, type_: repType, extraConfig: { table: n } }));
      } else if (kind === "sharepoint") {
        // 秘密フィールド（client_secret）はマスクされている可能性があるため、保存済み設定を使う。
        const resources = await invoke(
          "browse_graph_site_resources_for_source",
          { id: siblingId, overridesJson: JSON.stringify(shared) },
        );
        parentTableItems = resources.map((r) => ({
          key: `${r.kind}:${r.display_name}`,
          label: r.kind === "library" ? `${r.display_name}（ライブラリ）` : r.display_name,
          type_: r.kind === "library" ? "graph_sharepoint_library" : "graph_sharepoint_list",
          extraConfig: r.kind === "library" ? { library_name: r.display_name } : { list_name: r.display_name },
        }));
      } else if (kind === "notion") {
        const dbs = await invoke(
          "browse_notion_databases_for_source",
          { id: siblingId, overridesJson: JSON.stringify(shared) },
        );
        parentTableItems = dbs.map((d) => ({ key: d.id, label: d.title || "(無題)", type_: "notion", extraConfig: { database_id: d.id } }));
      } else if (kind === "airtable") {
        const tbls = await invoke(
          "browse_airtable_tables_for_source",
          { id: siblingId, overridesJson: JSON.stringify(shared), baseId: null },
        );
        // base_id/base_name は共有フィールド(token)には含めていないため、兄弟テーブルの設定から
        // 引き継ぐ。無いと新規テーブルがどの Base にも属さない扱いになり、汎用の「Airtable」
        // ツリーへ入ってしまう（実際に発生した不具合）。
        const siblingDetail = await invoke("get_source_detail", { id: siblingId }).catch(() => null);
        const baseId = siblingDetail ? String(siblingDetail.config.base_id ?? "") : "";
        const baseName = siblingDetail ? String(siblingDetail.config.base_name ?? "") : "";
        parentTableItems = tbls.map((t) => ({
          key: t.id,
          label: t.name ? `${t.name} [ID: ${t.id}]` : t.id,
          type_: "airtable",
          extraConfig: { table: t.id, table_name: t.name || t.id, base_id: baseId, base_name: baseName },
        }));
      } else if (kind === "graph365") {
        // API 取得ではなく、実装済みの種別のうち接続情報だけで使えるものを並べる。
        parentTableItems = GRAPH_ONE_CLICK_TYPES.map((t) => ({ key: t, label: sourceTypeLabel(t), type_: t, extraConfig: {} }));
      }
    } catch (e) {
      setPickerMessage(elements.tablesList, `取得失敗: ${String(e)}`);
      return;
    }

    // 既存の配下テーブルとの対応付け。
    parentTableMatched = new Map();
    if (kind === "graph365") {
      // 種別そのものが一覧の項目なので、config を読まずに対応付けできる。
      for (const c of children) {
        if (GRAPH_ONE_CLICK_TYPES.includes(c.type_) && !parentTableMatched.has(c.type_)) {
          parentTableMatched.set(c.type_, c.id);
        }
      }
    } else {
      const details = await Promise.all(
        children.map((c) => invoke("get_source_detail", { id: c.id }).catch(() => null)),
      );
      children.forEach((c, i) => {
        const cfg = details[i]?.config;
        if (!cfg) return;
        if (kind === "sqlite" || kind === "accdb" || kind === "excel_table") {
          if (typeof cfg.table === "string") parentTableMatched.set(cfg.table, c.id);
        } else if (kind === "sharepoint") {
          if (typeof cfg.list_name === "string") parentTableMatched.set(`list:${cfg.list_name}`, c.id);
          if (typeof cfg.library_name === "string") parentTableMatched.set(`library:${cfg.library_name}`, c.id);
        } else if (kind === "notion") {
          if (typeof cfg.database_id === "string") parentTableMatched.set(cfg.database_id, c.id);
        } else if (kind === "airtable") {
          if (typeof cfg.table === "string") {
            parentTableMatched.set(cfg.table, c.id);
          }
          if (typeof cfg.table_name === "string") {
            const matchByName = parentTableItems.find((item) => item.label.startsWith(cfg.table_name as string) || item.key === cfg.table_name);
            if (matchByName) parentTableMatched.set(matchByName.key, c.id);
          }
        }
      });
    }

    elements.tablesList.innerHTML = "";
    if (parentTableItems.length === 0) {
      setPickerMessage(elements.tablesList, "利用できるテーブルが見つかりませんでした");
    }
    for (const item of parentTableItems) {
      const row = document.createElement("label");
      row.className = "picker-item";
      const cb = document.createElement("input");
      cb.type = "checkbox";
      cb.dataset.key = item.key;
      cb.checked = parentTableMatched.has(item.key);
      cb.addEventListener("change", updateParentTablesCount);
      const span = document.createElement("span");
      span.textContent = item.label;
      row.append(cb, span);
      if (parentTableMatched.has(item.key)) {
        const badge = document.createElement("span");
        badge.className = "badge";
        badge.textContent = "取込済";
        row.appendChild(badge);
      }
      elements.tablesList.appendChild(row);
    }
    if (kind === "graph365") {
      const rest = GRAPH_PARAMETERIZED_TYPES.map(sourceTypeLabel).join(" / ");
      elements.tablesHint.textContent += `\n※ ${rest} は対象ユーザーやグループ ID の指定が必要なため、この一覧からは追加できません。`;
    }
    updateParentTablesCount();
  }

  async function openParentSettingsDialog(parentKey: string) {
    const children = appState.sources.filter((s) => s.parent === parentKey);
    if (children.length === 0) return;
    // 同じ接続先配下に複数の種別が混在する場合（例: 同一ブック内の excel_table と excel_sheets）は、
    // テーブル一覧の追加/削除に対応する種別を優先的に代表として選ぶ（表示種別・ボタン表示の一貫性のため）。
    const repChild = children.find((c) => parentBrowseKind(c.type_) !== null) ?? children[0];
    const repType = repChild.type_;
    const keys = sharedKeysFor(actions.parentKindOf(repType));
    const def = TYPE_DEFS.find((t) => t.value === repType);
    const fieldDefs = (def?.fields ?? []).filter((f) => keys.includes(f.key));

    elements.errorEl.textContent = "";
    elements.titleEl.textContent = `接続設定: ${actions.parentLabel(parentKey)}`;
    elements.typeLabelEl.textContent = `種別: ${parentKindLabel(actions.parentKindOf(repType), repType)}`;
    elements.guideBtn.hidden = actions.guideTextFor(repType) === null;
    elements.labelInput.value = appState.parentLabelOverrides.get(parentKey) ?? "";
    elements.scopeHintEl.textContent =
      `この接続情報は次の ${children.length} 個のテーブルに反映されます: ${children.map((c) => c.name).join(", ")}`;

    let prefill: Record<string, unknown> = {};
    setLinkedCredentials({});
    if (fieldDefs.length > 0) {
      try {
        const detail = await invoke("get_source_detail", { id: children[0].id });
        prefill = detail.config;
        setLinkedCredentials(detail.linked_credentials ?? {});
      } catch { /* noop */ }
    }
    renderFieldsInto(elements.dynamicEl, "pf-", fieldDefs, actions.fieldRenderer, repType, prefill);

    parentDialogState = { parentKey, keys, fieldDefs, children, repType };
    if (!elements.dialog.open) elements.dialog.showModal();
    await loadParentTables();
  }

  // ロック中でプレースホルダーが入っているパスワード欄をクリックしたら解錠を促す（本体ダイアログと同じ挙動）。
  elements.dynamicEl.addEventListener("focusin", async (ev) => {
    const el = ev.target as HTMLInputElement;
    if (el.type !== "password" || el.value !== SECRET_PLACEHOLDER || !parentDialogState) return;
    // 保存済みトークンへのリンク中は実値が無い（credentials 側にある）ため解錠を促す意味が無い。
    if (el.dataset.credLabel) return;
    const ok = await actions.ensureUnlocked();
    if (ok) {
      const detail = await invoke("get_source_detail", { id: parentDialogState.children[0].id });
      el.value = String((detail.config as Record<string, unknown>)[el.id.replace("pf-", "")] ?? "");
    }
  });

  elements.guideBtn.onclick = () => {
    if (parentDialogState) actions.openGuide(parentDialogState.repType);
  };

  elements.cancelBtn.onclick = () => elements.dialog.close();
  elements.closeXBtn.onclick = () => elements.dialog.close();

  elements.saveBtn.onclick = async () => {
    if (!parentDialogState) return;
    const { parentKey, keys, fieldDefs, children } = parentDialogState;
    elements.errorEl.textContent = "";
    const newLabel = elements.labelInput.value.trim();

    const sharedValues: Record<string, unknown> = {};
    for (const k of keys) {
      const el = document.getElementById(`pf-${k}`) as HTMLInputElement | null;
      if (!el) continue;
      const fdef = fieldDefs.find((f) => f.key === k);
      const raw = fieldRawValue(el, k);
      if (fdef?.optional && raw === "") continue; // 空の任意項目は既存値を維持（変更しない）
      sharedValues[k] = raw;
    }
    const requiredEmpty = keys.some((k) => {
      const fdef = fieldDefs.find((f) => f.key === k);
      if (fdef?.optional) return false;
      return !String(sharedValues[k] ?? "").trim();
    });
    if (fieldDefs.length > 0 && requiredEmpty) {
      elements.errorEl.textContent = "接続情報に未入力の項目があります";
      return;
    }

    // テーブル一覧のチェック差分。削除が発生する場合は先に確認を取る。
    const { toAdd, toRemove } = parentTableDiff();
    if (toRemove.length > 0) {
      const names = toRemove.map((t) => t.label).join("、");
      const ok = await confirmDelete(
        `チェックを外した ${toRemove.length} 個のテーブルを削除します。\n（${names}）\nキャッシュも消えます。よろしいですか？`,
      );
      if (!ok) return;
    }

    const prevLabel = appState.parentLabelOverrides.get(parentKey) ?? "";
    const labelChanged = newLabel !== prevLabel;
    if (toAdd.length === 0 && toRemove.length === 0 && !labelChanged) {
      let anySharedChange = false;
      for (const c of children) {
        const detail = await invoke("get_source_detail", { id: c.id });
        if (sharedConfigWouldChange(detail.config, sharedValues, keys, actions.normalizePath)) {
          anySharedChange = true;
          break;
        }
      }
      if (!anySharedChange) {
        elements.errorEl.textContent = "変更はありません";
        return;
      }
    }

    elements.saveBtn.disabled = true;
    elements.cancelBtn.disabled = true;
    elements.closeXBtn.disabled = true;
    elements.saveStatusEl.hidden = false;
    try {
      if (labelChanged) {
        await invoke("set_parent_label", { parentKey, label: newLabel });
        if (newLabel) appState.parentLabelOverrides.set(parentKey, newLabel);
        else appState.parentLabelOverrides.delete(parentKey);
      }

      for (const c of children) {
        const detail = await invoke("get_source_detail", { id: c.id });
        if (!sharedConfigWouldChange(detail.config, sharedValues, keys, actions.normalizePath)) continue;
        const cfg = { ...detail.config };
        for (const k of keys) {
          if (!(k in sharedValues)) continue;
          cfg[k] = sharedValues[k];
        }
        await invoke("update_source", { id: c.id, name: detail.name, configJson: JSON.stringify(cfg) });
      }

      // 追加を先に行う。接続情報の引き継ぎ元（children[0]）が削除対象だと参照できなくなるため。
      const failures: string[] = [];
      for (const item of toAdd) {
        const config: Record<string, unknown> = { ...item.extraConfig };
        for (const k of keys) {
          if (sharedValues[k] !== undefined) config[k] = sharedValues[k];
        }
        const dup = await findDuplicateSource(item.type_, config, appState.sources);
        if (dup) {
          if (window.confirm(duplicateConfirmMessage(dup.name))) {
            await invoke("reload_sources", { sourceIds: [dup.id] });
          }
          continue;
        }
        try {
          // 秘密フィールドがマスクされたままでも壊れないよう、既存テーブルの暗号化済み設定を引き継ぐ。
          await invoke("add_source_from_sibling", {
            siblingId: children[0].id,
            type: item.type_,
            name: item.label,
            configJson: JSON.stringify(config),
            sharedKeys: keys,
          });
        } catch (e) {
          failures.push(`${item.label}: ${String(e)}`);
        }
      }
      // チェックを外したテーブルを削除する。
      for (const t of toRemove) {
        await invoke("delete_source", { id: t.id });
        appState.selectedSourceIds.delete(t.id);
      }
      if (failures.length) {
        // 失敗があればダイアログは閉じずに理由を出す（サイレント失敗の防止）。
        await actions.loadSources();
        await actions.runSearch();
        await openParentSettingsDialog(parentKey);
        elements.errorEl.textContent = `${failures.length} 件の追加に失敗しました: ${failures.join(" / ")}`;
        return;
      }

      elements.dialog.close();
      await actions.loadSources();
      await actions.runSearch();
    } catch (e) {
      elements.errorEl.textContent = "保存に失敗: " + String(e);
    } finally {
      elements.saveBtn.disabled = false;
      elements.cancelBtn.disabled = false;
      elements.closeXBtn.disabled = false;
      elements.saveStatusEl.hidden = true;
    }
  };

  elements.tablesAllBtn.onclick = () => {
    elements.tablesList.querySelectorAll<HTMLInputElement>("input[type=checkbox]").forEach((cb) => { cb.checked = true; });
    updateParentTablesCount();
  };
  elements.tablesNoneBtn.onclick = () => {
    elements.tablesList.querySelectorAll<HTMLInputElement>("input[type=checkbox]").forEach((cb) => { cb.checked = false; });
    updateParentTablesCount();
  };
  elements.tablesRefreshBtn.onclick = () => void loadParentTables();

  return {
    openParentSettingsDialog,
    getRepType: () => parentDialogState?.repType ?? null,
    getSiblingSourceId: () => parentDialogState?.children[0]?.id ?? null,
    getParentSharedValues: parentSharedNow,
  };
}

export function mountParentSettingsDialogFromDom(
  actions: ParentSettingsDialogActions,
): ParentSettingsDialogController {
  return mountParentSettingsDialog({
    dialog: $<HTMLDialogElement>("parent-dialog"),
    titleEl: $<HTMLDivElement>("parent-dialog-title"),
    typeLabelEl: $<HTMLDivElement>("parent-type-label"),
    guideBtn: $<HTMLButtonElement>("parent-guide"),
    labelInput: $<HTMLInputElement>("parent-label"),
    scopeHintEl: $<HTMLDivElement>("parent-scope-hint"),
    dynamicEl: $<HTMLDivElement>("parent-dynamic"),
    errorEl: $<HTMLDivElement>("parent-error"),
    cancelBtn: $<HTMLButtonElement>("parent-cancel"),
    closeXBtn: $<HTMLButtonElement>("parent-close-x"),
    saveBtn: $<HTMLButtonElement>("parent-save"),
    saveStatusEl: $<HTMLSpanElement>("parent-save-status"),
    tablesFieldset: $<HTMLFieldSetElement>("parent-tables-fieldset"),
    tablesList: $<HTMLDivElement>("parent-tables-list"),
    tablesHint: $<HTMLDivElement>("parent-tables-hint"),
    tablesCount: $<HTMLSpanElement>("parent-tables-count"),
    tablesAccessBtn: $<HTMLButtonElement>("parent-tables-access"),
    tablesAllBtn: $<HTMLButtonElement>("parent-tables-all"),
    tablesNoneBtn: $<HTMLButtonElement>("parent-tables-none"),
    tablesRefreshBtn: $<HTMLButtonElement>("parent-tables-refresh"),
  }, actions);
}
