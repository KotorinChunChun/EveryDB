import { $, confirmDelete, setPickerMessage } from "../../app/dom";
import { invoke } from "../../app/ipc";
import { showToast } from "../../app/notifications";
import { writeClipboardText } from "../../shared/clipboard";
import type { CredentialScope, CredentialSummary } from "../../shared/types";
import { representativeTypeForCredentialScope as representativeTypeForScope } from "../../source-types/registry";
import { ENTRA_APPS_URL, NOTION_INTEGRATIONS_URL } from "../../source-types/forms";
import { SECRET_PLACEHOLDER } from "../source-editor/constants";
import { makeApiKeyButton, makeLinkButton } from "../source-editor/ui-buttons";
import type { WinAuthController } from "./winauth";

export interface CredentialManagerDeps {
  winAuth: WinAuthController;
  openGuide: (type: string) => void;
  openGuideDoc: (name: string) => void;
}

export interface CredentialManagerController {
  openCredentialManager: (initialScope?: CredentialScope) => Promise<void>;
  copyCredentialSecretToClipboard: (id: number) => Promise<void>;
  copyCredentialBundleAsJson: (
    id: number,
    secretKey: string,
    bundleFields: string[],
    idPrefix: string,
  ) => Promise<void>;
}

interface CredentialFieldDef {
  key: string;
  label: string;
  secret: boolean;
  helpUrl?: string;
  linkUrl?: string;
  linkLabel?: string;
  authorize?: {
    buttonLabel: string;
    usesFields: string[];
    provider: "box" | "graph";
  };
  selectOptions?: { value: string; label: string }[];
}

export function mountCredentialManager(deps: CredentialManagerDeps): CredentialManagerController {
  const { winAuth, openGuide, openGuideDoc } = deps;
  const { ensureUnlocked } = winAuth;

  let cmEditingId: number | null = null;
  let cmFormBaseline: Record<string, string> = {};
  let cmBoxAuthMethod: "oauth" | "ccg" = "oauth";
  let cmGraphAuthMethod: "app_only" | "delegated" = "app_only";
  let cmHiddenFieldStash: Record<string, string> = {};

  function credentialScopeFieldDefs(scope: CredentialScope): CredentialFieldDef[] {
    switch (scope) {
      case "notion":
        return [{ key: "token", label: "インテグレーショントークン", secret: true, helpUrl: NOTION_INTEGRATIONS_URL }];
      case "airtable":
        return [{
          key: "token", label: "Personal Access Token", secret: true,
          helpUrl: "https://airtable.com/create/tokens", linkUrl: "https://airtable.com/workspaces", linkLabel: "🌐 ワークスペース…",
        }];
      case "graph": {
        const common: CredentialFieldDef[] = [
          {
            key: "auth_method", label: "認証方式", secret: false,
            selectOptions: [
              { value: "app_only", label: "アプリケーションの許可（クライアントシークレット）" },
              { value: "delegated", label: "委任 OAuth（Microsoft にログインして認可）" },
            ],
          },
          { key: "tenant_id", label: "テナントID", secret: false, helpUrl: ENTRA_APPS_URL },
          { key: "client_id", label: "クライアントID", secret: false, helpUrl: ENTRA_APPS_URL },
        ];
        if (cmGraphAuthMethod === "delegated") {
          return [
            ...common,
            {
              key: "refresh_token", label: "Microsoft アカウントの認可", secret: true,
              authorize: {
                buttonLabel: "🔗 Microsoft にログインして認可…",
                usesFields: ["tenant_id", "client_id"],
                provider: "graph",
              },
            },
          ];
        }
        return [
          ...common,
          { key: "client_secret", label: "クライアントシークレット(値)", secret: true, helpUrl: ENTRA_APPS_URL },
        ];
      }
      case "box": {
        const BOX_DEV_CONSOLE = "https://app.box.com/developers/console";
        const common: CredentialFieldDef[] = [
          {
            key: "auth_method", label: "認証方式", secret: false,
            selectOptions: [
              { value: "oauth", label: "個人プラン（OAuth 2.0 ブラウザ認可）" },
              { value: "ccg", label: "法人/Enterpriseプラン（Client Credentials Grant・要管理者承認）" },
            ],
          },
          { key: "client_id", label: "クライアントID", secret: false, helpUrl: BOX_DEV_CONSOLE },
          { key: "client_secret", label: "クライアントシークレット", secret: true, helpUrl: BOX_DEV_CONSOLE },
        ];
        if (cmBoxAuthMethod === "ccg") {
          return [
            ...common,
            { key: "enterprise_id", label: "Enterprise ID", secret: false, helpUrl: "https://app.box.com/master/settings/account" },
          ];
        }
        return [
          ...common,
          {
            key: "refresh_token", label: "Box アカウントの認可", secret: true,
            authorize: {
              buttonLabel: "🔗 Box にログインして認可…",
              usesFields: ["client_id", "client_secret"],
              provider: "box",
            },
          },
        ];
      }
      case "postgres":
      case "mysql":
        return [
          { key: "user", label: "ユーザー", secret: false },
          { key: "password", label: "パスワード", secret: true },
        ];
    }
  }

  async function copyPlainValueToClipboard(value: string, message: string) {
    if (!value) return;
    const result = await writeClipboardText(value);
    showToast(result.ok ? message : "コピーに失敗しました");
  }

  function renderCredentialFormFields(scope: CredentialScope) {
    const container = $<HTMLDivElement>("cm-form-fields");
    container.innerHTML = "";
    for (const f of credentialScopeFieldDefs(scope)) {
      const label = document.createElement("label");
      label.textContent = f.label;
      const row = document.createElement("div");
      row.className = "path-row";

      if (f.selectOptions) {
        const select = document.createElement("select");
        select.id = `cmf-${f.key}`;
        for (const opt of f.selectOptions) {
          const optEl = document.createElement("option");
          optEl.value = opt.value;
          optEl.textContent = opt.label;
          select.appendChild(optEl);
        }
        select.value = scope === "graph" ? cmGraphAuthMethod : cmBoxAuthMethod;
        select.onchange = () => {
          for (const [key, value] of Object.entries(currentCmFormValues())) {
            if (key === "label" || key === f.key || !value) continue;
            cmHiddenFieldStash[key] = value;
          }
          if (scope === "graph") {
            cmGraphAuthMethod = select.value === "delegated" ? "delegated" : "app_only";
          } else {
            cmBoxAuthMethod = select.value === "ccg" ? "ccg" : "oauth";
          }
          renderCredentialFormFields(scope);
          for (const [key, value] of Object.entries(cmHiddenFieldStash)) {
            const el = document.getElementById(`cmf-${key}`) as HTMLInputElement | null;
            if (el && !el.value) {
              el.value = value;
              el.dispatchEvent(new Event("input"));
            }
          }
          updateCmSaveButtonState();
        };
        row.appendChild(select);
        label.appendChild(row);
        container.appendChild(label);
        continue;
      }

      const input = document.createElement("input");
      input.type = f.secret ? "password" : "text";
      input.autocomplete = "off";
      input.id = `cmf-${f.key}`;

      if (f.authorize) {
        input.hidden = true;
        row.appendChild(input);
        const status = document.createElement("span");
        status.className = "credential-profile-chip";
        const authBtn = document.createElement("button");
        authBtn.type = "button";
        authBtn.className = "btn browse-btn";
        authBtn.textContent = f.authorize.buttonLabel;
        const cancelBtn = document.createElement("button");
        cancelBtn.type = "button";
        cancelBtn.className = "btn browse-btn";
        cancelBtn.textContent = "取消";
        cancelBtn.hidden = true;
        cancelBtn.onclick = () => {
          const cmd = f.authorize!.provider === "graph" ? "graph_oauth_cancel" : "box_oauth_cancel";
          void invoke(cmd).catch(() => {});
        };
        const renderAuthStatus = () => {
          if (input.value) {
            status.textContent = "✅ 認可済み";
            status.classList.remove("unset");
          } else {
            status.textContent = "未認可";
            status.classList.add("unset");
          }
        };
        authBtn.onclick = async () => {
          const provider = f.authorize!.provider;
          const usedFields: Record<string, string> = {};
          for (const key of f.authorize!.usesFields) {
            const el = document.getElementById(`cmf-${key}`) as HTMLInputElement | null;
            usedFields[key] = el?.value.trim() ?? "";
          }
          if (provider === "box" && usedFields["client_secret"] === SECRET_PLACEHOLDER && cmEditingId !== null) {
            const unlocked = await ensureUnlocked();
            if (!unlocked) return;
            try {
              usedFields["client_secret"] = await invoke("credential_secret_value", { id: cmEditingId });
            } catch (e) {
              alert("クライアントシークレットの取得に失敗しました: " + String(e));
              return;
            }
          }
          if (Object.values(usedFields).some((v) => !v)) {
            alert(
              provider === "graph"
                ? "先にテナントID・クライアントIDを入力してください"
                : "先にクライアントID・クライアントシークレットを入力してください",
            );
            return;
          }
          authBtn.disabled = true;
          authBtn.textContent = "ブラウザで認可待ち…";
          cancelBtn.hidden = false;
          try {
            const res = provider === "graph"
              ? await invoke("graph_oauth_authorize", {
                tenantId: usedFields["tenant_id"],
                clientId: usedFields["client_id"],
              })
              : await invoke("box_oauth_authorize", {
                clientId: usedFields["client_id"],
                clientSecret: usedFields["client_secret"],
              });
            input.value = res.refresh_token;
            renderAuthStatus();
            updateCmSaveButtonState();
          } catch (e) {
            if (!String(e).includes("キャンセルしました")) {
              alert(
                (provider === "graph" ? "Microsoft の認可に失敗しました: " : "Box の認可に失敗しました: ")
                + String(e),
              );
            }
          } finally {
            authBtn.disabled = false;
            authBtn.textContent = f.authorize!.buttonLabel;
            cancelBtn.hidden = true;
          }
        };
        input.oninput = () => { renderAuthStatus(); updateCmSaveButtonState(); };
        renderAuthStatus();
        row.appendChild(status);
        row.appendChild(authBtn);
        row.appendChild(cancelBtn);
        label.appendChild(row);
        container.appendChild(label);
        continue;
      }

      row.appendChild(input);

      const copyBtn = document.createElement("button");
      copyBtn.type = "button";
      copyBtn.className = "btn browse-btn";
      copyBtn.id = `cmf-copy-${f.key}`;
      copyBtn.textContent = "📋 コピー";
      copyBtn.hidden = true;
      if (f.secret) {
        copyBtn.title = "現在の値をクリップボードにコピーします（本人確認を求められることがあります）";
        copyBtn.onclick = () => {
          const credentialId = copyBtn.dataset.credentialId;
          if (!credentialId) return;
          void copyCredentialSecretToClipboard(Number(credentialId));
        };
      } else {
        copyBtn.title = "現在の値をクリップボードにコピーします";
        copyBtn.onclick = () => void copyPlainValueToClipboard(input.value, `${f.label}をコピーしました`);
      }
      row.appendChild(copyBtn);
      if (f.helpUrl) row.appendChild(makeApiKeyButton(f.helpUrl));
      if (f.linkUrl) row.appendChild(makeLinkButton(f.linkUrl, f.linkLabel ?? "🌐 開く…"));
      label.appendChild(row);
      container.appendChild(label);
      input.oninput = () => updateCmSaveButtonState();
    }
  }

  function currentCmFormValues(): Record<string, string> {
    const values: Record<string, string> = { label: $<HTMLInputElement>("cm-label").value };
    const scope = $<HTMLSelectElement>("cm-scope").value as CredentialScope;
    for (const f of credentialScopeFieldDefs(scope)) {
      const el = document.getElementById(`cmf-${f.key}`) as HTMLInputElement | null;
      values[f.key] = el ? el.value : "";
    }
    return values;
  }

  function captureCmFormBaseline() {
    cmFormBaseline = currentCmFormValues();
    updateCmSaveButtonState();
  }

  function updateCmSaveButtonState() {
    const current = currentCmFormValues();
    const keys = new Set([...Object.keys(cmFormBaseline), ...Object.keys(current)]);
    let dirty = false;
    for (const k of keys) {
      if (cmFormBaseline[k] !== current[k]) { dirty = true; break; }
    }
    $<HTMLButtonElement>("cm-form-save").disabled = !dirty;
  }

  function resetCredentialForm() {
    cmEditingId = null;
    cmHiddenFieldStash = {};
    $<HTMLInputElement>("cm-label").value = "";
    const scope = $<HTMLSelectElement>("cm-scope").value as CredentialScope;
    if (scope === "box") cmBoxAuthMethod = "oauth";
    if (scope === "graph") cmGraphAuthMethod = "app_only";
    renderCredentialFormFields(scope);
    updateCredentialListSelection();
    captureCmFormBaseline();
  }

  async function startEditCredential(c: CredentialSummary) {
    cmEditingId = c.id;
    cmHiddenFieldStash = {};
    $<HTMLInputElement>("cm-label").value = c.label;
    const scope = c.scope as CredentialScope;
    let fields: Record<string, unknown> = {};
    try {
      const res = await invoke("credential_public_fields", { id: c.id });
      fields = res.fields;
    } catch { /* noop */ }
    if (scope === "box") cmBoxAuthMethod = fields["auth_method"] === "ccg" ? "ccg" : "oauth";
    if (scope === "graph") cmGraphAuthMethod = fields["auth_method"] === "delegated" ? "delegated" : "app_only";
    renderCredentialFormFields(scope);
    updateCredentialListSelection();
    for (const [k, v] of Object.entries(fields)) {
      const el = document.getElementById(`cmf-${k}`) as HTMLInputElement | null;
      if (el) el.value = String(v);
    }
    for (const f of credentialScopeFieldDefs(scope)) {
      if (f.secret) {
        const el = document.getElementById(`cmf-${f.key}`) as HTMLInputElement | null;
        if (el) {
          el.value = SECRET_PLACEHOLDER;
          el.dispatchEvent(new Event("input"));
        }
      }
      const copyBtn = document.getElementById(`cmf-copy-${f.key}`) as HTMLButtonElement | null;
      if (copyBtn) {
        copyBtn.hidden = false;
        copyBtn.dataset.credentialId = String(c.id);
      }
    }
    captureCmFormBaseline();
  }

  function updateCredentialListSelection() {
    const listEl = $<HTMLDivElement>("cm-list");
    for (const item of Array.from(listEl.querySelectorAll<HTMLElement>(".cm-profile-item"))) {
      const isNew = item.id === "cm-new";
      const selected = isNew ? cmEditingId === null : item.dataset.credentialId === String(cmEditingId);
      item.classList.toggle("selected", selected);
    }
  }

  async function refreshCredentialList() {
    const scope = $<HTMLSelectElement>("cm-scope").value;
    const listEl = $<HTMLDivElement>("cm-list");
    const errEl = $<HTMLDivElement>("cm-error");
    errEl.textContent = "";
    const newBtn = $<HTMLButtonElement>("cm-new");
    listEl.innerHTML = "";
    listEl.appendChild(newBtn);
    const loadingEl = document.createElement("div");
    loadingEl.className = "picker-empty";
    loadingEl.textContent = "読み込み中…";
    listEl.appendChild(loadingEl);
    try {
      const items = await invoke("list_credentials", { scope });
      loadingEl.remove();
      if (items.length === 0) {
        const emptyEl = document.createElement("div");
        emptyEl.className = "picker-empty";
        emptyEl.textContent = "まだ保存済みトークンがありません";
        listEl.appendChild(emptyEl);
      }
      for (const c of items) {
        const item = document.createElement("div");
        item.className = "cm-profile-item";
        item.dataset.credentialId = String(c.id);
        item.tabIndex = 0;
        item.onclick = () => void startEditCredential(c);
        item.onkeydown = (ev) => { if (ev.key === "Enter" || ev.key === " ") { ev.preventDefault(); void startEditCredential(c); } };

        const name = document.createElement("span");
        name.className = "cm-profile-name";
        name.textContent = `${c.label}（作成: ${c.created_at}${c.used_by.length ? ` / 使用中: ${c.used_by.length}件` : ""}）`;
        item.appendChild(name);

        const delBtn = document.createElement("button");
        delBtn.type = "button";
        delBtn.className = "btn danger";
        delBtn.textContent = "削除";
        delBtn.onclick = async (ev) => {
          ev.stopPropagation();
          if (c.used_by.length > 0) {
            alert(
              `${c.used_by.length}件のソースで使用中のため削除できません: ${c.used_by.map((u) => u.source_name).join("、")}\n` +
              "先に別のトークンへ付け替えるか、それらのソースを削除してください",
            );
            return;
          }
          if (!(await confirmDelete(`「${c.label}」を削除しますか？`))) return;
          try {
            await invoke("delete_credential", { id: c.id });
            if (cmEditingId === c.id) resetCredentialForm();
            await refreshCredentialList();
          } catch (e) {
            errEl.textContent = "削除に失敗: " + String(e);
          }
        };
        item.appendChild(delBtn);
        listEl.appendChild(item);
        if (c.used_by.length > 0) {
          const usageEl = document.createElement("div");
          usageEl.className = "cm-profile-usage";
          usageEl.textContent = `使用中の接続先: ${c.used_by.map((u) => u.source_name).join("、")}`;
          listEl.appendChild(usageEl);
        }
      }
      updateCredentialListSelection();
    } catch (e) {
      loadingEl.remove();
      setPickerMessage(listEl, `取得失敗: ${String(e)}`);
      listEl.insertBefore(newBtn, listEl.firstChild);
    }
  }

  async function openCredentialManager(initialScope?: CredentialScope) {
    if (initialScope) $<HTMLSelectElement>("cm-scope").value = initialScope;
    resetCredentialForm();
    $<HTMLDialogElement>("credential-manager").showModal();
    await refreshCredentialList();
  }

  async function copyCredentialSecretToClipboard(id: number) {
    const ok = await ensureUnlocked();
    if (!ok) return;
    try {
      const value = await invoke("credential_secret_value", { id });
      const copied = await writeClipboardText(value);
      showToast(copied.ok ? "トークンをコピーしました" : "コピーに失敗しました: " + String(copied.error));
    } catch (e) {
      showToast("コピーに失敗しました: " + String(e));
    }
  }

  async function copyCredentialBundleAsJson(
    id: number,
    secretKey: string,
    bundleFields: string[],
    idPrefix: string,
  ) {
    const ok = await ensureUnlocked();
    if (!ok) return;
    try {
      const secretValue = await invoke("credential_secret_value", { id });
      const obj: Record<string, string> = {};
      for (const bf of bundleFields) {
        const el = document.getElementById(`${idPrefix}${bf}`) as HTMLInputElement | null;
        obj[bf] = el?.value ?? "";
      }
      obj[secretKey] = secretValue;
      const copied = await writeClipboardText(JSON.stringify(obj, null, 2));
      showToast(copied.ok ? "トークンをコピーしました" : "コピーに失敗しました: " + String(copied.error));
    } catch (e) {
      showToast("コピーに失敗しました: " + String(e));
    }
  }

  $<HTMLInputElement>("cm-label").oninput = () => updateCmSaveButtonState();
  $<HTMLButtonElement>("cm-form-save").onclick = async () => {
    const errEl = $<HTMLDivElement>("cm-error");
    errEl.textContent = "";
    const scope = $<HTMLSelectElement>("cm-scope").value as CredentialScope;
    const label = $<HTMLInputElement>("cm-label").value.trim();
    if (!label) {
      errEl.textContent = "名前を入力してください";
      return;
    }
    const defs = credentialScopeFieldDefs(scope);
    const fields: Record<string, unknown> = {};
    for (const f of defs) {
      const el = document.getElementById(`cmf-${f.key}`) as HTMLInputElement | null;
      if (el) fields[f.key] = el.value.trim();
    }
    if (scope === "graph") fields["auth_method"] = cmGraphAuthMethod;
    if (scope === "box") fields["auth_method"] = cmBoxAuthMethod;
    if (cmEditingId === null) {
      const missing = defs.some((f) => {
        if (f.key === "auth_method") return false;
        if (f.authorize) return !String(fields[f.key] ?? "").trim();
        return !String(fields[f.key] ?? "").trim();
      });
      if (missing) {
        errEl.textContent = defs.some((f) => f.authorize && !String(fields[f.key] ?? "").trim())
          ? "先にログインして認可を完了してください"
          : "すべての項目を入力してください";
        return;
      }
    }
    try {
      let savedId: number;
      if (cmEditingId !== null) {
        await invoke("update_credential", { id: cmEditingId, label, fieldsJson: JSON.stringify(fields) });
        savedId = cmEditingId;
      } else {
        savedId = await invoke("add_credential", { scope, label, fieldsJson: JSON.stringify(fields) });
      }
      await refreshCredentialList();
      await startEditCredential({ id: savedId, scope, label, created_at: "", used_by: [] });
    } catch (e) {
      errEl.textContent = "保存に失敗: " + String(e);
    }
  };
  $<HTMLButtonElement>("cm-new").onclick = () => resetCredentialForm();
  $<HTMLSelectElement>("cm-scope").onchange = () => {
    resetCredentialForm();
    void refreshCredentialList();
  };
  $<HTMLButtonElement>("cm-guide").onclick = () => {
    const scope = $<HTMLSelectElement>("cm-scope").value as CredentialScope;
    if (scope === "graph" && cmGraphAuthMethod === "delegated") {
      openGuideDoc("microsoft365-delegated");
      return;
    }
    openGuide(representativeTypeForScope(scope));
  };
  $<HTMLButtonElement>("credential-manager-close").onclick = () => $<HTMLDialogElement>("credential-manager").close();
  $<HTMLButtonElement>("credential-manager-close-x").onclick = () => $<HTMLDialogElement>("credential-manager").close();
  $<HTMLButtonElement>("s-credential-manager").onclick = () => void openCredentialManager();

  return {
    openCredentialManager,
    copyCredentialSecretToClipboard,
    copyCredentialBundleAsJson,
  };
}
