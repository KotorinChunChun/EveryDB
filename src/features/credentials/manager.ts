import { $, confirmDelete, setPickerMessage } from "../../app/dom";
import { invoke } from "../../app/ipc";
import { showToast } from "../../app/notifications";
import { writeClipboardText } from "../../shared/clipboard";
import type { CredentialScope, CredentialSummary } from "../../shared/types";
import {
  isGraphCapability,
  representativeTypeForCredentialScope as representativeTypeForScope,
  type GraphCapability,
} from "../../source-types/registry";
import { ENTRA_APPS_URL, NOTION_INTEGRATIONS_URL } from "../../source-types/forms";
import { SECRET_PLACEHOLDER } from "../source-editor/constants";
import { makeApiKeyButton, makeLinkButton } from "../source-editor/ui-buttons";
import type { WinAuthController } from "./winauth";
import {
  populateCredentialPermissionStatus,
  supportsCredentialPermissionInspection,
} from "./permissions";

export interface CredentialManagerDeps {
  winAuth: WinAuthController;
  openGuide: (type: string) => void;
  openGuideDoc: (name: string) => void;
}

export function graphGuideDocument(authMethod: "app_only" | "delegated"): string {
  return authMethod === "delegated" ? "microsoft365-entra-user" : "microsoft365-entra-app";
}

export const GRAPH_CAPABILITY_OPTIONS: readonly {
  value: GraphCapability;
  label: string;
  scopes: string;
}[] = [
  { value: "directory_users", label: "Entra ID ユーザー一覧", scopes: "User.Read.All" },
  { value: "directory_groups", label: "Entra ID グループ一覧", scopes: "Group.Read.All" },
  { value: "directory_devices", label: "Entra ID デバイス一覧", scopes: "Device.Read.All" },
  { value: "intune_devices", label: "Intune デバイス一覧", scopes: "DeviceManagementManagedDevices.Read.All" },
  { value: "licenses", label: "Microsoft 365 ライセンス一覧", scopes: "LicenseAssignment.Read.All" },
  { value: "sharepoint", label: "SharePoint サイト・リスト / Teams選択", scopes: "Sites.Read.All, Team.ReadBasic.All" },
  { value: "calendar_self", label: "本人の Outlook 予定表", scopes: "Calendars.ReadBasic" },
  { value: "onedrive_self", label: "本人の OneDrive", scopes: "Files.Read" },
];

export function graphCapabilitiesFromFields(fields: Record<string, unknown>): GraphCapability[] {
  const values = Array.isArray(fields.capabilities) ? fields.capabilities : null;
  if (!values && fields.auth_method === "delegated") return ["sharepoint"];
  const supported = (values ?? []).filter(isGraphCapability);
  return supported.length > 0 ? supported : ["sharepoint"];
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
  hidden?: boolean;
  helpUrl?: string;
  linkUrl?: string;
  linkLabel?: string;
  authorize?: {
    buttonLabel: string;
    usesFields: string[];
    provider: "box" | "graph" | "microsoft_account";
  };
  selectOptions?: { value: string; label: string }[];
  capabilityOptions?: typeof GRAPH_CAPABILITY_OPTIONS;
}

export function mountCredentialManager(deps: CredentialManagerDeps): CredentialManagerController {
  const { winAuth, openGuide, openGuideDoc } = deps;
  const { ensureUnlocked } = winAuth;

  let cmEditingId: number | null = null;
  let cmFormBaseline: Record<string, string> = {};
  let cmBoxAuthMethod: "oauth" | "ccg" = "oauth";
  let cmGraphAuthMethod: "app_only" | "delegated" = "app_only";
  let cmGraphCapabilities: GraphCapability[] = ["sharepoint"];
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
              key: "capabilities",
              label: "利用する機能（必要な権限だけ選択）",
              secret: false,
              capabilityOptions: GRAPH_CAPABILITY_OPTIONS,
            },
            {
              key: "refresh_token", label: "Microsoft アカウントの認可", secret: true,
              authorize: {
                buttonLabel: "🔗 Microsoft にログインして認可…",
                usesFields: ["tenant_id", "client_id"],
                provider: "graph",
              },
            },
            { key: "authorized_account_name", label: "", secret: false, hidden: true },
            { key: "authorized_account_id", label: "", secret: false, hidden: true },
          ];
        }
        return [
          ...common,
          { key: "client_secret", label: "クライアントシークレット(値)", secret: true, helpUrl: ENTRA_APPS_URL },
        ];
      }
      case "microsoft_account":
        return [
          {
            key: "refresh_token", label: "Microsoft アカウントの認可", secret: true,
            authorize: {
              buttonLabel: "🔗 個人 Microsoft アカウントで認可…",
              usesFields: [],
              provider: "microsoft_account",
            },
          },
          { key: "authorized_account_name", label: "", secret: false, hidden: true },
          { key: "authorized_account_id", label: "", secret: false, hidden: true },
        ];
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
      if (f.hidden) {
        const hidden = document.createElement("input");
        hidden.type = "hidden";
        hidden.id = `cmf-${f.key}`;
        container.appendChild(hidden);
        continue;
      }
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
            if (cmGraphAuthMethod === "delegated" && cmGraphCapabilities.length === 0) {
              cmGraphCapabilities = ["sharepoint"];
            }
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

      if (f.capabilityOptions) {
        const hidden = document.createElement("input");
        hidden.type = "hidden";
        hidden.id = `cmf-${f.key}`;
        const options = document.createElement("div");
        options.className = "credential-capability-options";
        const syncValue = () => {
          hidden.value = JSON.stringify(cmGraphCapabilities);
        };
        for (const option of f.capabilityOptions) {
          const optionLabel = document.createElement("label");
          optionLabel.className = "check credential-capability-option";
          const checkbox = document.createElement("input");
          checkbox.type = "checkbox";
          checkbox.value = option.value;
          checkbox.checked = cmGraphCapabilities.includes(option.value);
          checkbox.onchange = () => {
            cmGraphCapabilities = f.capabilityOptions!
              .filter((candidate) => (
                options.querySelector<HTMLInputElement>(`input[value="${candidate.value}"]`)?.checked
              ))
              .map((candidate) => candidate.value);
            syncValue();
            cmHiddenFieldStash.refresh_token = "";
            const refreshToken = document.getElementById("cmf-refresh_token") as HTMLInputElement | null;
            if (refreshToken) {
              refreshToken.value = "";
              refreshToken.dispatchEvent(new Event("input"));
            }
            for (const key of ["authorized_account_name", "authorized_account_id"]) {
              const accountField = document.getElementById(`cmf-${key}`) as HTMLInputElement | null;
              if (accountField) accountField.value = "";
            }
            updateCmSaveButtonState();
          };
          const text = document.createElement("span");
          text.textContent = `${option.label}（${option.scopes}）`;
          optionLabel.append(checkbox, text);
          options.appendChild(optionLabel);
        }
        syncValue();
        const hint = document.createElement("small");
        hint.className = "roles-hint";
        hint.textContent = "選択を変更すると再認可が必要です。共通で User.Read / offline_access を要求します。Entra ID・Intune・ライセンス一覧の権限には管理者同意が必要で、サインインユーザーのロールやIntuneライセンスも適用されます。";
        row.append(hidden, options);
        label.append(row, hint);
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
        status.className = "credential-profile-chip credential-authorization-status";
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
          const cmd = f.authorize!.provider === "box" ? "box_oauth_cancel" : "graph_oauth_cancel";
          void invoke(cmd).catch(() => {});
        };
        const renderAuthStatus = () => {
          if (input.value) {
            if (f.authorize!.provider === "graph" || f.authorize!.provider === "microsoft_account") {
              const accountName = (document.getElementById("cmf-authorized_account_name") as HTMLInputElement | null)
                ?.value.trim() ?? "";
              const accountId = (document.getElementById("cmf-authorized_account_id") as HTMLInputElement | null)
                ?.value.trim() ?? "";
              status.textContent = accountName || accountId
                ? `✅ 認可済み: ${accountName || "表示IDなし"}（アカウントID: ${accountId || "未取得"}）`
                : "✅ 認可済み（認可アカウント未記録）";
            } else {
              status.textContent = "✅ 認可済み";
            }
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
                capabilities: [...cmGraphCapabilities],
              })
              : provider === "microsoft_account"
                ? await invoke("microsoft_account_oauth_authorize")
                : await invoke("box_oauth_authorize", {
                  clientId: usedFields["client_id"],
                  clientSecret: usedFields["client_secret"],
                });
            input.value = res.refresh_token;
            if (provider === "graph" || provider === "microsoft_account") {
              const accountName = document.getElementById("cmf-authorized_account_name") as HTMLInputElement | null;
              const accountId = document.getElementById("cmf-authorized_account_id") as HTMLInputElement | null;
              if (accountName) accountName.value = res.authorized_account_name ?? "";
              if (accountId) accountId.value = res.authorized_account_id ?? "";
            }
            renderAuthStatus();
            updateCmSaveButtonState();
          } catch (e) {
            if (!String(e).includes("キャンセルしました")) {
              alert(
                (provider === "box" ? "Box の認可に失敗しました: " : "Microsoft の認可に失敗しました: ")
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
    if (scope === "graph") {
      cmGraphAuthMethod = "app_only";
      cmGraphCapabilities = ["sharepoint"];
    }
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
    const personalReauthorizationRequired = scope === "microsoft_account"
      && fields["_requires_reauthorization"] === true;
    delete fields["_requires_reauthorization"];
    $<HTMLDivElement>("cm-error").textContent = personalReauthorizationRequired
      ? "認証設定の更新により再認可が必要です。個人 Microsoft アカウントで認可し直して保存してください。"
      : "";
    if (scope === "box") cmBoxAuthMethod = fields["auth_method"] === "ccg" ? "ccg" : "oauth";
    if (scope === "graph") {
      cmGraphAuthMethod = fields["auth_method"] === "delegated" ? "delegated" : "app_only";
      cmGraphCapabilities = graphCapabilitiesFromFields(fields);
    }
    renderCredentialFormFields(scope);
    updateCredentialListSelection();
    for (const [k, v] of Object.entries(fields)) {
      if (scope === "graph" && k === "capabilities") continue;
      const el = document.getElementById(`cmf-${k}`) as HTMLInputElement | null;
      if (el) el.value = String(v);
    }
    for (const f of credentialScopeFieldDefs(scope)) {
      if (f.secret) {
        const el = document.getElementById(`cmf-${f.key}`) as HTMLInputElement | null;
        if (el) {
          el.value = personalReauthorizationRequired && f.key === "refresh_token"
            ? ""
            : SECRET_PLACEHOLDER;
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

        const main = document.createElement("span");
        main.className = "cm-profile-main";
        const name = document.createElement("span");
        name.className = "cm-profile-name";
        name.textContent = `${c.label}（作成: ${c.created_at}${c.used_by.length ? ` / 使用中: ${c.used_by.length}件` : ""}）`;
        main.appendChild(name);
        if (supportsCredentialPermissionInspection(c.scope)) {
          const permissionStatus = document.createElement("small");
          permissionStatus.className = "credential-permission-status";
          main.appendChild(permissionStatus);
          void populateCredentialPermissionStatus(c.id, permissionStatus);
        }
        if (c.used_by.length > 0) {
          const usageEl = document.createElement("small");
          usageEl.className = "cm-profile-usage";
          usageEl.textContent = `使用中の接続先: ${c.used_by.map((u) => u.source_name).join("、")}`;
          main.appendChild(usageEl);
        }
        item.appendChild(main);

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
    if (scope === "graph") {
      fields["auth_method"] = cmGraphAuthMethod;
      if (cmGraphAuthMethod === "delegated") fields["capabilities"] = [...cmGraphCapabilities];
      else delete fields["capabilities"];
    }
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
    if (scope === "graph" && cmGraphAuthMethod === "delegated") {
      if (cmGraphCapabilities.length === 0) {
        errEl.textContent = "利用する機能を1つ以上選択してください";
        return;
      }
      if (!String(fields["refresh_token"] ?? "").trim()) {
        errEl.textContent = "capability に合わせて Microsoft の認可を実行してください";
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
    if (scope === "graph") {
      openGuideDoc(graphGuideDocument(cmGraphAuthMethod));
      return;
    }
    if (scope === "microsoft_account") {
      openGuideDoc("microsoft365-personal");
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
