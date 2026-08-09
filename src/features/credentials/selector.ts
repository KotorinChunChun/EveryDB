import { $, setPickerMessage } from "../../app/dom";
import { invoke } from "../../app/ipc";
import type { CredentialScope, CredentialSummary } from "../../shared/types";
import type { FieldDef } from "../../source-types/forms";
import {
  graphCredentialRequirementsFor,
  type GraphAuthMethod,
  type GraphCapability,
} from "../../source-types/registry";
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
    sourceType: string,
  ) => void;
}

export interface GraphCredentialCompatibility {
  compatible: boolean;
  authMethod: GraphAuthMethod | "unknown";
  capabilities: GraphCapability[];
  reason: string | null;
}

export function graphUsesSignedInUser(
  sourceType: string,
  fields: Record<string, unknown>,
): boolean {
  return (sourceType === "graph_calendar" || sourceType === "graph_onedrive")
    && fields.auth_method === "delegated";
}

export function updateGraphSelfSubjectUi(
  sourceType: string,
  idPrefix: string,
  fields: Record<string, unknown>,
): void {
  if (sourceType !== "graph_calendar" && sourceType !== "graph_onedrive") return;
  const input = document.getElementById(`${idPrefix}user`) as HTMLInputElement | null;
  if (!input) return;
  const delegated = graphUsesSignedInUser(sourceType, fields);
  input.disabled = delegated;
  let hint = document.getElementById(`${idPrefix}graph-self-subject-hint`);
  if (!hint) {
    hint = document.createElement("small");
    hint.id = `${idPrefix}graph-self-subject-hint`;
    hint.className = "roles-hint graph-self-subject-hint";
    input.closest("label")?.appendChild(hint);
  }
  hint.textContent = delegated ? "委任 OAuth: サインインした本人を取得します" : "";
  hint.hidden = !delegated;
  if (delegated) input.setAttribute("aria-describedby", hint.id);
  else input.removeAttribute("aria-describedby");
}

export function graphCredentialCompatibility(
  sourceType: string,
  fields: Record<string, unknown>,
): GraphCredentialCompatibility {
  const requirements = graphCredentialRequirementsFor(sourceType);
  const rawMethod = fields.auth_method;
  const authMethod: GraphAuthMethod | "unknown" = rawMethod === undefined || rawMethod === null || rawMethod === "app_only"
    ? "app_only"
    : rawMethod === "delegated"
      ? "delegated"
      : "unknown";
  const capabilities = Array.isArray(fields.capabilities)
    ? fields.capabilities.filter((value): value is GraphCapability => (
      value === "sharepoint" || value === "calendar_self" || value === "onedrive_self"
    ))
    : authMethod === "delegated"
      ? ["sharepoint" as const]
      : [];

  if (!requirements) return { compatible: true, authMethod, capabilities, reason: null };
  if (authMethod === "unknown" || !requirements.authMethods.includes(authMethod)) {
    return {
      compatible: false,
      authMethod,
      capabilities,
      reason: authMethod === "delegated"
        ? "このデータソースは委任 OAuth に対応していません。アプリケーション認証を選択してください。"
        : "この資格情報の認証方式には対応していません。",
    };
  }
  if (authMethod === "delegated") {
    const missing = requirements.capabilities.filter((capability) => !capabilities.includes(capability));
    if (missing.length > 0) {
      return {
        compatible: false,
        authMethod,
        capabilities,
        reason: `必要な capability: ${missing.join(", ")}。トークン管理画面で追加して再認可してください。`,
      };
    }
  }
  return { compatible: true, authMethod, capabilities, reason: null };
}

function credentialScopeLabel(scope: CredentialScope): string {
  switch (scope) {
    case "notion": return "Notion";
    case "airtable": return "Airtable";
    case "graph": return "Microsoft Entra ID（組織）";
    case "microsoft_account": return "Microsoft アカウント（個人）";
    case "box": return "Box";
    case "postgres": return "PostgreSQL";
    case "mysql": return "MySQL";
  }
}

export function mountCredentialSelector(deps: CredentialSelectorDeps): CredentialSelectorController {
  const { getSourceEditor, manager } = deps;
  const { openCredentialManager, copyCredentialSecretToClipboard, copyCredentialBundleAsJson } = manager;

  let lastPickerScope: CredentialScope | null = null;

  async function renderCredentialPickerList(
    items: CredentialSummary[],
    sourceType: string,
    onPick: (id: number, label: string, fields: Record<string, unknown>) => void,
  ) {
    const listEl = $<HTMLDivElement>("credential-select-picker-list");
    listEl.innerHTML = "";
    if (items.length === 0) {
      listEl.innerHTML = "<div class='picker-empty'>該当する保存済みトークンがありません。下の「🔑 管理画面を開く…」から追加できます</div>";
      return;
    }
    const entries = await Promise.all(items.map(async (summary) => {
      try {
        return { summary, details: await invoke("credential_public_fields", { id: summary.id }), error: null };
      } catch (error) {
        return { summary, details: null, error: String(error) };
      }
    }));
    for (const { summary: c, details, error } of entries) {
      const item = document.createElement("button");
      item.type = "button";
      item.className = "picker-item credential-picker-item";
      const title = document.createElement("span");
      title.textContent = `${c.label}（作成: ${c.created_at}）`;
      item.appendChild(title);

      const compatibility = details && c.scope === "graph"
        ? graphCredentialCompatibility(sourceType, details.fields)
        : null;
      const reason = error ? `詳細を取得できません: ${error}` : compatibility?.reason ?? null;
      if (reason) {
        item.disabled = true;
        item.title = reason;
        const hint = document.createElement("small");
        hint.className = "credential-compatibility-reason";
        hint.textContent = reason;
        item.appendChild(hint);
      } else if (compatibility) {
        const hint = document.createElement("small");
        hint.className = "credential-compatibility-summary";
        hint.textContent = compatibility.authMethod === "delegated"
          ? `委任 OAuth / ${compatibility.capabilities.join(", ")}`
          : "アプリケーション認証";
        item.appendChild(hint);
      }
      item.onclick = () => {
        if (!details || item.disabled) return;
        onPick(c.id, details.label, details.fields);
        $<HTMLDialogElement>("credential-select-picker").close();
      };
      listEl.appendChild(item);
    }
  }

  async function openCredentialSelectPicker(
    scope: CredentialScope,
    sourceType: string,
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
      await renderCredentialPickerList(items, sourceType, onPick);
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
    sourceType: string,
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
      updateGraphSelfSubjectUi(sourceType, idPrefix, fields);
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
        copyBtn.hidden = scope === "microsoft_account";
      } else {
        chip.textContent = "未選択";
        chip.classList.add("unset");
        pickBtn.textContent = "選択…";
        copyBtn.hidden = true;
      }
    }

    pickBtn.onclick = () => openCredentialSelectPicker(scope, sourceType, applyPick);
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
    const knownId = currentCredentialId();
    if (scope === "graph" && knownId !== null) {
      void invoke("credential_public_fields", { id: knownId }).then((details) => {
        if (currentCredentialId() === knownId) {
          updateGraphSelfSubjectUi(sourceType, idPrefix, details.fields);
        }
      }).catch(() => {});
    }
  }

  return { wireCredentialSelector };
}
