//! データセル・接続先を既定アプリで開くための安全境界。

use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

/// フロントエンドの getCellUrl と共通の、アプリ内から開いてよい URL scheme。
const OPENABLE_URL_SCHEMES: &[&str] = &[
    "http:",
    "https:",
    "file:",
    "notion:",
    "ftp:",
    "mailto:",
    "teams:",
    "obsidian:",
];

#[derive(Debug, PartialEq, Eq)]
enum OpenTarget<'a> {
    Url(&'a str),
    Path(&'a str),
}

fn looks_like_windows_drive_path(target: &str) -> bool {
    let bytes = target.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'\\' | b'/')
}

fn has_uri_scheme(target: &str) -> bool {
    let bytes = target.as_bytes();
    let Some(colon) = bytes.iter().position(|b| *b == b':') else {
        return false;
    };
    colon > 0
        && bytes[0].is_ascii_alphabetic()
        && bytes[..colon]
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(*b, b'+' | b'-' | b'.'))
}

fn is_openable_url(target: &str) -> bool {
    let lower = target.to_ascii_lowercase();
    OPENABLE_URL_SCHEMES
        .iter()
        .any(|scheme| lower.starts_with(scheme))
        && match lower.split_once(':') {
            Some(("mailto", rest)) => !rest.is_empty() && !rest.contains(char::is_whitespace),
            Some((_, rest)) => rest.starts_with("//"),
            None => false,
        }
}

fn is_file_url(target: &str) -> bool {
    target.len() >= 7 && target[..7].eq_ignore_ascii_case("file://")
}

fn classify_target(target: &str) -> Result<OpenTarget<'_>, String> {
    let target = target.trim();
    if target.is_empty() {
        return Err("開く対象が空です".to_string());
    }
    if is_openable_url(target) {
        return Ok(OpenTarget::Url(target));
    }
    if has_uri_scheme(target) && !looks_like_windows_drive_path(target) {
        return Err("許可されていないURLスキームです".to_string());
    }
    Ok(OpenTarget::Path(target))
}

/// 「開く」で実行を許さない拡張子。open_ref の対象文字列はデータソース側の内容
/// （CSV の 1 セル等）に由来するため、汚染されたデータで実行ファイルが起動されうる。
const DANGEROUS_OPEN_EXTS: &[&str] = &[
    "exe", "com", "scr", "pif", "bat", "cmd", "msi", "msp", "cpl", "hta", "jar", "reg", "lnk",
    "ps1", "psm1", "vbs", "vbe", "js", "jse", "wsf", "wsh", "sh", "url", "inf", "chm",
];

fn percent_decode_lossy(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = &value[i + 1..i + 3];
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                out.push(byte as char);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

fn target_filename_for_extension(target: &str) -> String {
    let path_part = target.split(['?', '#']).next().unwrap_or(target);
    let decoded = percent_decode_lossy(path_part);
    decoded
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(&decoded)
        .split(':')
        .next()
        .unwrap_or(&decoded)
        .trim_end_matches([' ', '.'])
        .to_string()
}

/// 対象パスの拡張子が実行系なら Err を返す。URLでは query/fragment・%エンコード・
/// Windowsが無視する末尾空白/ドット、NTFS ADSを除去してから拡張子を判定する。
fn reject_executable_target(target: &str) -> Result<(), String> {
    let filename = target_filename_for_extension(target);
    let ext = std::path::Path::new(&filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.trim_end_matches([' ', '.']).to_ascii_lowercase());
    if let Some(ext) = ext {
        if DANGEROUS_OPEN_EXTS.contains(&ext.as_str()) {
            return Err(format!(
                "安全のため、実行ファイル形式（.{ext}）はアプリからは開きません。必要ならエクスプローラーから開いてください"
            ));
        }
    }
    Ok(())
}
#[tauri::command]
pub fn open_ref(app: AppHandle, target: String) -> Result<(), String> {
    match classify_target(&target)? {
        OpenTarget::Url(url) => {
            if is_file_url(url) {
                reject_executable_target(url)?;
            }
            app.opener()
                .open_url(url, None::<&str>)
                .map_err(|error| error.to_string())
        }
        OpenTarget::Path(path) => {
            reject_executable_target(path)?;
            app.opener()
                .open_path(path, None::<&str>)
                .map_err(|error| error.to_string())
        }
    }
}

#[cfg(test)]
mod open_ref_tests {
    use super::{classify_target, is_openable_url, reject_executable_target, OpenTarget};

    #[test]
    fn frontend_and_backend_agree_on_openable_url_schemes() {
        for target in [
            "https://example.com",
            "HTTP://example.com",
            "file:///C:/data/report.xlsx",
            "notion://www.notion.so/page",
            "ftp://example.com/archive",
            "mailto:user@example.com",
            "teams://chat/19:example",
            "obsidian://open?vault=notes",
        ] {
            assert!(is_openable_url(target), "expected URL scheme: {target}");
        }
        for target in [
            "javascript:alert(1)",
            "C:\\data\\report.xlsx",
            "www.example.com",
        ] {
            assert!(
                !is_openable_url(target),
                "expected non-URL target: {target}"
            );
        }
    }

    #[test]
    fn file_url_keeps_executable_extension_protection() {
        assert!(reject_executable_target("file:///C:/tmp/payload.exe").is_err());
        assert!(reject_executable_target("file:///C:/data/report.xlsx").is_ok());
    }

    #[test]
    fn rejects_executables_regardless_of_case() {
        assert!(reject_executable_target(r"\\attacker\share\payload.EXE").is_err());
        assert!(reject_executable_target(r"C:\tmp\a.bat").is_err());
        assert!(reject_executable_target(r"C:\tmp\a.ps1").is_err());
        assert!(reject_executable_target(r"C:\tmp\shortcut.lnk").is_err());
    }

    #[test]
    fn allows_ordinary_documents_and_extensionless_paths() {
        assert!(reject_executable_target(r"C:\data\customers.csv").is_ok());
        assert!(reject_executable_target(r"C:\data\employees.xlsx").is_ok());
        assert!(reject_executable_target(r"C:\data\folder").is_ok());
    }
    #[test]
    fn rejects_unknown_uri_schemes_without_treating_them_as_paths() {
        for target in [
            "javascript:alert(1)",
            "ms-settings:privacy",
            "shell:AppsFolder",
        ] {
            assert!(
                classify_target(target).is_err(),
                "expected rejection: {target}"
            );
        }
        assert_eq!(
            classify_target(r"C:\data\report.xlsx"),
            Ok(OpenTarget::Path(r"C:\data\report.xlsx"))
        );
        assert_eq!(
            classify_target(r"\\server\share\report.xlsx"),
            Ok(OpenTarget::Path(r"\\server\share\report.xlsx"))
        );
    }

    #[test]
    fn rejects_encoded_and_windows_normalized_executable_extensions() {
        for target in [
            "file:///C:/tmp/payload%2Eexe",
            "file:///C:/tmp/payload.EXE.",
            r"C:\tmp\payload.cmd   ",
            r"C:\tmp\payload.ps1:alternate-stream",
        ] {
            assert!(
                reject_executable_target(target).is_err(),
                "expected rejection: {target}"
            );
        }
    }

    #[test]
    fn accepts_only_well_formed_allowed_urls() {
        for target in ["https:example.com", "file:C:/tmp/report.xlsx", "mailto:"] {
            assert!(!is_openable_url(target), "expected malformed URL: {target}");
        }
        assert_eq!(
            classify_target("https:example.com").unwrap_err(),
            "許可されていないURLスキームです"
        );
    }
}
