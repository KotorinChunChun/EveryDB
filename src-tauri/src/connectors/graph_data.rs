//! Microsoft Graph API を利用する connector 群。

use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::Value;

use super::common::{make_record, CommonCfg};
use super::Connector;
use crate::graph::GraphClient;
use crate::model::Record;
// ---- Microsoft Graph 系（要件 M5 / EntraID ユーザー・グループ・Teams）----
#[derive(Deserialize)]
struct GraphAuth {
    tenant_id: String,
    client_id: String,
    client_secret: String,
}

/// Graph の JSON スカラー値（$select したフィールド）を表示用文字列へ。
pub(crate) fn json_scalar_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Array(a) => a
            .iter()
            .map(json_scalar_to_string)
            .collect::<Vec<_>>()
            .join(", "),
        _ => String::new(),
    }
}

/// Graph のフラットなオブジェクト（user/group 等）を共通 Record へ。
pub(crate) fn graph_flat_record(
    obj: &Value,
    fields: &[&str],
    label_field: &str,
    open_ref: Option<String>,
    common: &CommonCfg,
) -> Record {
    let pairs: Vec<(String, String)> = fields
        .iter()
        .map(|f| {
            (
                (*f).to_string(),
                obj.get(*f).map(json_scalar_to_string).unwrap_or_default(),
            )
        })
        .collect();
    let mut rec = make_record(pairs, "", common);
    if common.label_column.is_none() {
        rec.label = obj
            .get(label_field)
            .and_then(|v| v.as_str())
            .map(String::from);
    }
    if common.open_ref_column.is_none() {
        rec.open_ref = open_ref;
    }
    rec
}

pub(crate) fn mailto_open_ref(obj: &Value) -> Option<String> {
    obj.get("mail")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|m| format!("mailto:{m}"))
}

// EntraID ユーザー
#[derive(Deserialize)]
pub(crate) struct GraphUsersConnector {
    #[serde(flatten)]
    auth: GraphAuth,
    #[serde(flatten)]
    common: CommonCfg,
}
impl Connector for GraphUsersConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let client = GraphClient::new(
            &self.auth.tenant_id,
            &self.auth.client_id,
            &self.auth.client_secret,
        )?;
        let url = "https://graph.microsoft.com/v1.0/users?$select=displayName,userPrincipalName,mail,jobTitle,department,officeLocation,mobilePhone,id&$top=999";
        let fields = [
            "displayName",
            "userPrincipalName",
            "mail",
            "jobTitle",
            "department",
            "officeLocation",
            "mobilePhone",
        ];
        Ok(client
            .get_all(url)?
            .iter()
            .map(|u| graph_flat_record(u, &fields, "displayName", mailto_open_ref(u), &self.common))
            .collect())
    }
}

// EntraID グループ
#[derive(Deserialize)]
pub(crate) struct GraphGroupsConnector {
    #[serde(flatten)]
    auth: GraphAuth,
    #[serde(flatten)]
    common: CommonCfg,
}
impl Connector for GraphGroupsConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let client = GraphClient::new(
            &self.auth.tenant_id,
            &self.auth.client_id,
            &self.auth.client_secret,
        )?;
        let url = "https://graph.microsoft.com/v1.0/groups?$select=displayName,mail,description,visibility,id&$top=999";
        let fields = ["displayName", "mail", "description", "visibility"];
        Ok(client
            .get_all(url)?
            .iter()
            .map(|g| graph_flat_record(g, &fields, "displayName", mailto_open_ref(g), &self.common))
            .collect())
    }
}

// Teams チームリスト（Team 化されたグループ）
#[derive(Deserialize)]
pub(crate) struct GraphTeamsConnector {
    #[serde(flatten)]
    auth: GraphAuth,
    #[serde(flatten)]
    common: CommonCfg,
}
impl Connector for GraphTeamsConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let client = GraphClient::new(
            &self.auth.tenant_id,
            &self.auth.client_id,
            &self.auth.client_secret,
        )?;
        let url = "https://graph.microsoft.com/v1.0/groups?$filter=resourceProvisioningOptions/Any(x:x eq 'Team')&$select=displayName,mail,description,id&$top=999";
        let fields = ["displayName", "mail", "description"];
        Ok(client
            .get_all(url)?
            .iter()
            .map(|t| graph_flat_record(t, &fields, "displayName", None, &self.common))
            .collect())
    }
}

// ---- SharePoint 共通（サイト ID 解決は crate::graph::site_id に集約）----
use crate::graph::site_id as graph_site_id;

/// SharePoint リスト項目の fields から、システム列を除いた表示対象かを判定。
fn sp_field_is_display(key: &str) -> bool {
    if key.starts_with('_') || key.starts_with('@') {
        return false;
    }
    const BLOCK: &[&str] = &[
        "ContentType",
        "Attachments",
        "Edit",
        "LinkTitleNoMenu",
        "LinkTitle",
        "LinkTitle2",
        "ItemChildCount",
        "FolderChildCount",
        "AuthorLookupId",
        "EditorLookupId",
        "AppAuthorLookupId",
        "AppEditorLookupId",
    ];
    !BLOCK.contains(&key)
}

// ---- SharePoint リスト ----
#[derive(Deserialize)]
pub(crate) struct GraphSharePointListConnector {
    #[serde(flatten)]
    auth: GraphAuth,
    site_url: String,
    list_name: String,
    #[serde(flatten)]
    common: CommonCfg,
}
impl Connector for GraphSharePointListConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let client = GraphClient::new(
            &self.auth.tenant_id,
            &self.auth.client_id,
            &self.auth.client_secret,
        )?;
        let sid = graph_site_id(&client, &self.site_url)?;
        // リスト名（内部名 or 表示名）から id を解決。
        let lists = client.get(&format!(
            "https://graph.microsoft.com/v1.0/sites/{sid}/lists?$select=id,name,displayName"
        ))?;
        let lid = lists
            .get("value")
            .and_then(|v| v.as_array())
            .and_then(|arr| {
                arr.iter().find(|l| {
                    let n = l.get("name").and_then(|v| v.as_str());
                    let d = l.get("displayName").and_then(|v| v.as_str());
                    n == Some(self.list_name.as_str()) || d == Some(self.list_name.as_str())
                })
            })
            .and_then(|l| l.get("id").and_then(|v| v.as_str()))
            .ok_or_else(|| anyhow!("リストが見つかりません: {}", self.list_name))?
            .to_string();

        let url = format!(
            "https://graph.microsoft.com/v1.0/sites/{sid}/lists/{lid}/items?expand=fields&$top=200"
        );
        let mut out = Vec::new();
        for it in client.get_all(&url)? {
            let web = it.get("webUrl").and_then(|v| v.as_str()).map(String::from);
            let mut pairs = Vec::new();
            let fields = it.get("fields").and_then(|v| v.as_object());
            if let Some(f) = fields {
                for (k, v) in f {
                    if sp_field_is_display(k) {
                        pairs.push((k.clone(), json_scalar_to_string(v)));
                    }
                }
            }
            let mut rec = make_record(pairs, web.as_deref().unwrap_or(""), &self.common);
            if self.common.open_ref_column.is_none() {
                rec.open_ref = web;
            }
            if self.common.label_column.is_none() {
                if let Some(title) = fields.and_then(|f| f.get("Title")).and_then(|v| v.as_str()) {
                    rec.label = Some(title.to_string());
                }
            }
            out.push(rec);
        }
        Ok(out)
    }
}

// ---- SharePoint ドキュメントライブラリ ----
#[derive(Deserialize)]
pub(crate) struct GraphSharePointLibraryConnector {
    #[serde(flatten)]
    auth: GraphAuth,
    site_url: String,
    #[serde(default)]
    library_name: Option<String>,
    #[serde(flatten)]
    common: CommonCfg,
}
impl Connector for GraphSharePointLibraryConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let client = GraphClient::new(
            &self.auth.tenant_id,
            &self.auth.client_id,
            &self.auth.client_secret,
        )?;
        let sid = graph_site_id(&client, &self.site_url)?;
        // ライブラリ名指定があれば drives から一致検索、無ければ既定ライブラリ /drive。
        let did = match self.library_name.as_deref().filter(|s| !s.is_empty()) {
            Some(name) => {
                let drives = client.get(&format!(
                    "https://graph.microsoft.com/v1.0/sites/{sid}/drives?$select=id,name"
                ))?;
                drives
                    .get("value")
                    .and_then(|v| v.as_array())
                    .and_then(|arr| {
                        arr.iter()
                            .find(|d| d.get("name").and_then(|v| v.as_str()) == Some(name))
                    })
                    .and_then(|d| d.get("id").and_then(|v| v.as_str()))
                    .ok_or_else(|| anyhow!("ライブラリが見つかりません: {name}"))?
                    .to_string()
            }
            None => client
                .get(&format!(
                    "https://graph.microsoft.com/v1.0/sites/{sid}/drive?$select=id"
                ))?
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("既定ライブラリを取得できません"))?
                .to_string(),
        };

        let mut out = Vec::new();
        walk_drive(
            &client,
            &format!("sites/{sid}/drives/{did}"),
            "root",
            &self.common,
            &mut out,
        )?;
        Ok(out)
    }
}

/// ドライブ配下を再帰的に走査してファイル/フォルダをレコード化する。
/// `drive_base` は "sites/{sid}/drives/{did}" または "users/{upn}/drive" のような
/// Graph の drive リソースパス（末尾スラッシュ無し）。
fn walk_drive(
    client: &GraphClient,
    drive_base: &str,
    item_id: &str,
    common: &CommonCfg,
    out: &mut Vec<Record>,
) -> Result<()> {
    let url =
        format!("https://graph.microsoft.com/v1.0/{drive_base}/items/{item_id}/children?$top=200");
    for c in client.get_all(&url)? {
        let is_folder = c.get("folder").is_some();
        let name = c
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let web = c.get("webUrl").and_then(|v| v.as_str()).map(String::from);
        let size = c.get("size").map(json_scalar_to_string).unwrap_or_default();
        let modified = c
            .get("lastModifiedDateTime")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let path = c
            .get("parentReference")
            .and_then(|p| p.get("path"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let pairs = vec![
            ("名前".to_string(), name.clone()),
            (
                "種類".to_string(),
                if is_folder {
                    "フォルダ"
                } else {
                    "ファイル"
                }
                .to_string(),
            ),
            ("サイズ".to_string(), size),
            ("更新日時".to_string(), modified),
            ("パス".to_string(), path),
        ];
        let mut rec = make_record(pairs, web.as_deref().unwrap_or(""), common);
        if common.open_ref_column.is_none() {
            rec.open_ref = web;
        }
        if common.label_column.is_none() {
            rec.label = Some(name);
        }
        out.push(rec);

        if is_folder {
            if let Some(id) = c.get("id").and_then(|v| v.as_str()) {
                walk_drive(client, drive_base, id, common, out)?;
            }
        }
    }
    Ok(())
}

// ---- Entra ID デバイス一覧（要件: M365 & デバイス/ライセンス管理系）----
#[derive(Deserialize)]
pub(crate) struct GraphDevicesConnector {
    #[serde(flatten)]
    auth: GraphAuth,
    #[serde(flatten)]
    common: CommonCfg,
}
impl Connector for GraphDevicesConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let client = GraphClient::new(
            &self.auth.tenant_id,
            &self.auth.client_id,
            &self.auth.client_secret,
        )?;
        let url = "https://graph.microsoft.com/v1.0/devices?$select=displayName,operatingSystem,operatingSystemVersion,isCompliant,trustType,approximateLastSignInDateTime&$top=999";
        let fields = [
            "displayName",
            "operatingSystem",
            "operatingSystemVersion",
            "isCompliant",
            "trustType",
            "approximateLastSignInDateTime",
        ];
        Ok(client
            .get_all(url)?
            .iter()
            .map(|d| graph_flat_record(d, &fields, "displayName", None, &self.common))
            .collect())
    }
}

// ---- Intune デバイス一覧 ----
#[derive(Deserialize)]
pub(crate) struct GraphIntuneDevicesConnector {
    #[serde(flatten)]
    auth: GraphAuth,
    #[serde(flatten)]
    common: CommonCfg,
}
impl Connector for GraphIntuneDevicesConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let client = GraphClient::new(
            &self.auth.tenant_id,
            &self.auth.client_id,
            &self.auth.client_secret,
        )?;
        let url = "https://graph.microsoft.com/v1.0/deviceManagement/managedDevices?$select=deviceName,operatingSystem,osVersion,complianceState,userPrincipalName,lastSyncDateTime&$top=999";
        let fields = [
            "deviceName",
            "operatingSystem",
            "osVersion",
            "complianceState",
            "userPrincipalName",
            "lastSyncDateTime",
        ];
        Ok(client
            .get_all(url)?
            .iter()
            .map(|d| graph_flat_record(d, &fields, "deviceName", None, &self.common))
            .collect())
    }
}

// ---- Microsoft 365 ライセンス一覧 ----
#[derive(Deserialize)]
pub(crate) struct GraphLicensesConnector {
    #[serde(flatten)]
    auth: GraphAuth,
    #[serde(flatten)]
    common: CommonCfg,
}
impl Connector for GraphLicensesConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let client = GraphClient::new(
            &self.auth.tenant_id,
            &self.auth.client_id,
            &self.auth.client_secret,
        )?;
        let url = "https://graph.microsoft.com/v1.0/subscribedSkus?$select=skuPartNumber,skuId,consumedUnits,prepaidUnits,capabilityStatus";
        let mut out = Vec::new();
        for sku in client.get_all(url)? {
            let part_number = sku
                .get("skuPartNumber")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let consumed = sku
                .get("consumedUnits")
                .map(json_scalar_to_string)
                .unwrap_or_default();
            let enabled = sku
                .get("prepaidUnits")
                .and_then(|p| p.get("enabled"))
                .map(json_scalar_to_string)
                .unwrap_or_default();
            let remaining = match (enabled.parse::<i64>(), consumed.parse::<i64>()) {
                (Ok(e), Ok(c)) => (e - c).to_string(),
                _ => String::new(),
            };
            let pairs = vec![
                ("ライセンス名".to_string(), part_number.clone()),
                (
                    "SKU ID".to_string(),
                    sku.get("skuId")
                        .map(json_scalar_to_string)
                        .unwrap_or_default(),
                ),
                ("総数".to_string(), enabled),
                ("消費数".to_string(), consumed),
                ("残数".to_string(), remaining),
                (
                    "状態".to_string(),
                    sku.get("capabilityStatus")
                        .map(json_scalar_to_string)
                        .unwrap_or_default(),
                ),
            ];
            let mut rec = make_record(pairs, "", &self.common);
            if self.common.label_column.is_none() {
                rec.label = Some(part_number);
            }
            out.push(rec);
        }
        Ok(out)
    }
}

// ---- Outlook 予定表（要件: 対象ユーザーの UPN を指定）----
#[derive(Deserialize)]
pub(crate) struct GraphCalendarConnector {
    #[serde(flatten)]
    auth: GraphAuth,
    /// 対象ユーザーの UPN（メールアドレス）。
    user: String,
    #[serde(flatten)]
    common: CommonCfg,
}
impl Connector for GraphCalendarConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let client = GraphClient::new(
            &self.auth.tenant_id,
            &self.auth.client_id,
            &self.auth.client_secret,
        )?;
        let user = urlencoding_light(&self.user);
        let url = format!(
            "https://graph.microsoft.com/v1.0/users/{user}/events?$select=subject,start,end,location,organizer,attendees,webLink&$top=250&$orderby=start/dateTime desc"
        );
        let mut out = Vec::new();
        for ev in client.get_all(&url)? {
            let subject = ev
                .get("subject")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let start = ev
                .get("start")
                .and_then(|s| s.get("dateTime"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let end = ev
                .get("end")
                .and_then(|s| s.get("dateTime"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let location = ev
                .get("location")
                .and_then(|l| l.get("displayName"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let organizer = ev
                .get("organizer")
                .and_then(|o| o.get("emailAddress"))
                .and_then(|e| e.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let attendees = ev
                .get("attendees")
                .and_then(|a| a.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|a| a.get("emailAddress")?.get("name")?.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            let web_link = ev.get("webLink").and_then(|v| v.as_str()).map(String::from);
            let pairs = vec![
                ("件名".to_string(), subject.clone()),
                ("開始".to_string(), start),
                ("終了".to_string(), end),
                ("場所".to_string(), location),
                ("主催者".to_string(), organizer),
                ("参加者".to_string(), attendees),
            ];
            let mut rec = make_record(pairs, "", &self.common);
            if self.common.label_column.is_none() {
                rec.label = Some(subject);
            }
            if self.common.open_ref_column.is_none() {
                rec.open_ref = web_link;
            }
            out.push(rec);
        }
        Ok(out)
    }
}

// ---- OneDrive（対象ユーザーの UPN を指定。既存 walk_drive を再利用）----
#[derive(Deserialize)]
pub(crate) struct GraphOneDriveConnector {
    #[serde(flatten)]
    auth: GraphAuth,
    /// 対象ユーザーの UPN（メールアドレス）。
    user: String,
    #[serde(flatten)]
    common: CommonCfg,
}
impl Connector for GraphOneDriveConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let client = GraphClient::new(
            &self.auth.tenant_id,
            &self.auth.client_id,
            &self.auth.client_secret,
        )?;
        let user = urlencoding_light(&self.user);
        let mut out = Vec::new();
        walk_drive(
            &client,
            &format!("users/{user}/drive"),
            "root",
            &self.common,
            &mut out,
        )?;
        Ok(out)
    }
}

// ---- Teams タスク & Planner（グループ ID 単位で配下の全プランのタスクを集約）----
#[derive(Deserialize)]
pub(crate) struct GraphPlannerConnector {
    #[serde(flatten)]
    auth: GraphAuth,
    /// タスクを読みたい Microsoft 365 グループ（チーム）の ID。
    group_id: String,
    #[serde(flatten)]
    common: CommonCfg,
}
impl Connector for GraphPlannerConnector {
    fn fetch(&self) -> Result<Vec<Record>> {
        let client = GraphClient::new(
            &self.auth.tenant_id,
            &self.auth.client_id,
            &self.auth.client_secret,
        )?;
        let plans = client.get(&format!(
            "https://graph.microsoft.com/v1.0/groups/{}/planner/plans?$select=id,title",
            self.group_id
        ))?;
        let mut out = Vec::new();
        let Some(plans) = plans.get("value").and_then(|v| v.as_array()) else {
            return Ok(out);
        };
        for plan in plans {
            let plan_id = plan
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let plan_title = plan
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if plan_id.is_empty() {
                continue;
            }
            // bucket(列) の ID → 名前 の対応表を先に作る。
            let buckets = client.get(&format!(
                "https://graph.microsoft.com/v1.0/planner/plans/{plan_id}/buckets?$select=id,name"
            ))?;
            let bucket_names: std::collections::HashMap<String, String> = buckets
                .get("value")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|b| {
                            Some((
                                b.get("id")?.as_str()?.to_string(),
                                b.get("name")?.as_str()?.to_string(),
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default();

            let tasks_url = format!(
                "https://graph.microsoft.com/v1.0/planner/plans/{plan_id}/tasks?$select=title,dueDateTime,percentComplete,bucketId,assignments"
            );
            for t in client.get_all(&tasks_url)? {
                let title = t
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let due = t
                    .get("dueDateTime")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let percent = t
                    .get("percentComplete")
                    .map(json_scalar_to_string)
                    .unwrap_or_default();
                let status = match percent.as_str() {
                    "100" => "完了",
                    "0" => "未着手",
                    "" => "",
                    _ => "進行中",
                };
                let bucket = t
                    .get("bucketId")
                    .and_then(|v| v.as_str())
                    .and_then(|id| bucket_names.get(id))
                    .cloned()
                    .unwrap_or_default();
                let assignee_count = t
                    .get("assignments")
                    .and_then(|a| a.as_object())
                    .map(|o| o.len())
                    .unwrap_or(0);
                let pairs = vec![
                    ("プラン".to_string(), plan_title.clone()),
                    ("タスク名".to_string(), title.clone()),
                    ("Bucket".to_string(), bucket),
                    ("期限".to_string(), due),
                    ("進行状況".to_string(), status.to_string()),
                    ("担当者数".to_string(), assignee_count.to_string()),
                ];
                let mut rec = make_record(pairs, "", &self.common);
                if self.common.label_column.is_none() {
                    rec.label = Some(title);
                }
                out.push(rec);
            }
        }
        Ok(out)
    }
}

/// UPN 等をパス埋め込み用に最小限エスケープ（英数字・@・.・-・_ 以外を %XX に）。
pub(crate) fn urlencoding_light(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'@' | b'.' | b'-' | b'_' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod graph_live_tests {
    use super::*;
    use crate::connectors::build_connector;

    fn env(k: &str) -> Option<String> {
        std::env::var(k).ok().filter(|s| !s.is_empty())
    }

    // ライブ結合テスト。環境変数を設定して実行:
    //   $env:EVERYDB_GRAPH_TENANT=..; $env:EVERYDB_GRAPH_CLIENT=..; $env:EVERYDB_GRAPH_SECRET=..;
    //   $env:EVERYDB_SP_SITE='https://.../sites/..'; cargo test -- --ignored graph_live
    #[test]
    #[ignore = "requires EVERYDB_GRAPH_* env vars + admin-consented app"]
    fn graph_live_all() {
        let tenant = env("EVERYDB_GRAPH_TENANT").expect("EVERYDB_GRAPH_TENANT");
        let client = env("EVERYDB_GRAPH_CLIENT").expect("EVERYDB_GRAPH_CLIENT");
        let secret = env("EVERYDB_GRAPH_SECRET").expect("EVERYDB_GRAPH_SECRET");
        let auth = serde_json::json!({ "tenant_id": tenant, "client_id": client, "client_secret": secret });

        let run = |type_: &str, extra: serde_json::Value| -> Vec<Record> {
            let mut cfg = auth.clone();
            for (k, v) in extra.as_object().unwrap() {
                cfg[k] = v.clone();
            }
            build_connector(type_, &cfg.to_string())
                .unwrap()
                .fetch()
                .unwrap()
        };

        let users = run("graph_entra_users", serde_json::json!({}));
        println!("users = {}", users.len());
        assert!(!users.is_empty());

        let groups = run("graph_entra_groups", serde_json::json!({}));
        println!("groups = {}", groups.len());
        assert!(!groups.is_empty());

        let teams = run("graph_teams", serde_json::json!({}));
        println!("teams = {}", teams.len());
        assert!(!teams.is_empty());

        if let Some(site) = env("EVERYDB_SP_SITE") {
            let list = run(
                "graph_sharepoint_list",
                serde_json::json!({ "site_url": site, "list_name": "SampleList" }),
            );
            println!("sharepoint list = {}", list.len());
            assert!(!list.is_empty());
            assert!(list.iter().all(|r| r
                .open_ref
                .as_deref()
                .map(|u| u.starts_with("http"))
                .unwrap_or(false)));

            let lib = run(
                "graph_sharepoint_library",
                serde_json::json!({ "site_url": site }),
            );
            println!("sharepoint library = {}", lib.len());
            assert!(!lib.is_empty());
        }
    }
}
