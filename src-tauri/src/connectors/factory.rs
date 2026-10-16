//! Connector trait と種別別 factory。

use anyhow::{anyhow, Result};
use serde_json::Value;

use crate::model::Record;

use super::cloud::{
    AirtableConnector, BoxConnector, GoogleSheetsConnector, IcalConnector, NotionConnector,
};
use super::database::{MysqlConnector, PostgresConnector};
use super::graph_data::{
    GraphCalendarConnector, GraphDevicesConnector, GraphGroupsConnector,
    GraphIntuneDevicesConnector, GraphLicensesConnector, GraphOneDriveConnector,
    GraphPlannerConnector, GraphSharePointLibraryConnector, GraphSharePointListConnector,
    GraphTeamsConnector, GraphUsersConnector, MicrosoftAccountOneDriveConnector,
};
use super::local::{
    AccessConnector, BrowserBookmarksConnector, BrowserDownloadsConnector, BrowserHistoryConnector,
    CsvConnector, ExcelRangeConnector, ExcelSheetsConnector, ExcelTableConnector, FolderConnector,
    ImeCfg, ImeConnector, ImeFlavor, SqliteConnector,
};
use super::public_data::{
    CorporateNumberConnector, PokeApiConnector, PostalCodeConnector, WebPageConnector,
};
use super::strip_path_quotes;

/// データソース取得の共通拡張点（要件 §5-3）。
pub trait Connector {
    fn fetch(&self) -> Result<Vec<Record>>;
    fn field_names(&self) -> Result<Vec<String>> {
        let records = self.fetch()?;
        let mut seen = std::collections::HashSet::new();
        let mut names = Vec::new();
        for r in &records {
            for k in r.fields.keys() {
                if seen.insert(k.clone()) {
                    names.push(k.clone());
                }
            }
        }
        Ok(names)
    }
}

pub const SUPPORTED_SOURCE_TYPES: &[&str] = &[
    "csv",
    "sqlite",
    "accdb",
    "excel_table",
    "excel_range",
    "excel_cell",
    "excel_sheets",
    "folder",
    "ime_msime",
    "ime_google",
    "notion",
    "graph_entra_users",
    "graph_entra_groups",
    "graph_teams",
    "graph_sharepoint_list",
    "graph_sharepoint_library",
    "graph_entra_devices",
    "graph_intune_devices",
    "graph_licenses",
    "graph_calendar",
    "graph_onedrive",
    "microsoft_account_onedrive",
    "graph_planner",
    "browser_history",
    "browser_bookmarks",
    "browser_downloads",
    "google_sheets",
    "ical_calendar",
    "airtable",
    "postgres",
    "mysql",
    "box_files",
    "postal_code",
    "corporate_number",
    "web_page",
    "pokeapi",
];

pub fn is_supported_source_type(type_: &str) -> bool {
    SUPPORTED_SOURCE_TYPES.contains(&type_)
}

fn normalize_path_in_config(config_json: &str) -> Result<String> {
    let mut v: Value = serde_json::from_str(config_json)?;
    let mut changed = false;
    if let Some(obj) = v.as_object_mut() {
        for key in ["path", "key_path"] {
            if let Some(p) = obj.get(key).and_then(|x| x.as_str()) {
                let cleaned = strip_path_quotes(p);
                obj.insert(key.to_string(), Value::String(cleaned));
                changed = true;
            }
        }
    }
    if changed {
        Ok(serde_json::to_string(&v)?)
    } else {
        Ok(config_json.to_string())
    }
}

pub fn build_connector(type_: &str, config_json: &str) -> Result<Box<dyn Connector>> {
    use serde_json::from_str;
    if !is_supported_source_type(type_) {
        return Err(anyhow!("未対応のデータソース種別: {type_}"));
    }
    let config_json = crate::secret::unprotect_config(config_json)?;
    let config_json = normalize_path_in_config(&config_json)?;
    let config_json = config_json.as_str();
    match type_ {
        "csv" => Ok(Box::new(from_str::<CsvConnector>(config_json)?)),
        "sqlite" => Ok(Box::new(from_str::<SqliteConnector>(config_json)?)),
        "excel_table" => Ok(Box::new(from_str::<ExcelTableConnector>(config_json)?)),
        "excel_range" | "excel_cell" => Ok(Box::new(from_str::<ExcelRangeConnector>(config_json)?)),
        "excel_sheets" => Ok(Box::new(from_str::<ExcelSheetsConnector>(config_json)?)),
        "folder" => Ok(Box::new(from_str::<FolderConnector>(config_json)?)),
        "ime_msime" => Ok(Box::new(ImeConnector::new(
            from_str::<ImeCfg>(config_json)?,
            ImeFlavor::MsIme,
        ))),
        "ime_google" => Ok(Box::new(ImeConnector::new(
            from_str::<ImeCfg>(config_json)?,
            ImeFlavor::Google,
        ))),
        "accdb" => Ok(Box::new(from_str::<AccessConnector>(config_json)?)),
        "notion" => Ok(Box::new(from_str::<NotionConnector>(config_json)?)),
        "graph_entra_users" => Ok(Box::new(from_str::<GraphUsersConnector>(config_json)?)),
        "graph_entra_groups" => Ok(Box::new(from_str::<GraphGroupsConnector>(config_json)?)),
        "graph_teams" => Ok(Box::new(from_str::<GraphTeamsConnector>(config_json)?)),
        "graph_sharepoint_list" => Ok(Box::new(from_str::<GraphSharePointListConnector>(
            config_json,
        )?)),
        "graph_sharepoint_library" => Ok(Box::new(from_str::<GraphSharePointLibraryConnector>(
            config_json,
        )?)),
        "browser_history" => Ok(Box::new(from_str::<BrowserHistoryConnector>(config_json)?)),
        "browser_bookmarks" => Ok(Box::new(from_str::<BrowserBookmarksConnector>(
            config_json,
        )?)),
        "browser_downloads" => Ok(Box::new(from_str::<BrowserDownloadsConnector>(
            config_json,
        )?)),
        "graph_entra_devices" => Ok(Box::new(from_str::<GraphDevicesConnector>(config_json)?)),
        "graph_intune_devices" => Ok(Box::new(from_str::<GraphIntuneDevicesConnector>(
            config_json,
        )?)),
        "graph_licenses" => Ok(Box::new(from_str::<GraphLicensesConnector>(config_json)?)),
        "graph_calendar" => Ok(Box::new(from_str::<GraphCalendarConnector>(config_json)?)),
        "graph_onedrive" => Ok(Box::new(from_str::<GraphOneDriveConnector>(config_json)?)),
        "microsoft_account_onedrive" => Ok(Box::new(
            from_str::<MicrosoftAccountOneDriveConnector>(config_json)?,
        )),
        "graph_planner" => Ok(Box::new(from_str::<GraphPlannerConnector>(config_json)?)),
        "google_sheets" => Ok(Box::new(from_str::<GoogleSheetsConnector>(config_json)?)),
        "ical_calendar" => Ok(Box::new(from_str::<IcalConnector>(config_json)?)),
        "airtable" => Ok(Box::new(from_str::<AirtableConnector>(config_json)?)),
        "postgres" => Ok(Box::new(from_str::<PostgresConnector>(config_json)?)),
        "mysql" => Ok(Box::new(from_str::<MysqlConnector>(config_json)?)),
        "box_files" => Ok(Box::new(from_str::<BoxConnector>(config_json)?)),
        "postal_code" => Ok(Box::new(from_str::<PostalCodeConnector>(config_json)?)),
        "corporate_number" => Ok(Box::new(from_str::<CorporateNumberConnector>(config_json)?)),
        "web_page" => Ok(Box::new(from_str::<WebPageConnector>(config_json)?)),
        "pokeapi" => Ok(Box::new(from_str::<PokeApiConnector>(config_json)?)),
        other => Err(anyhow!(
            "factory定義が不足しているデータソース種別: {other}"
        )),
    }
}
