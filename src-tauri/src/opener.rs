//! データセル・接続先を既定アプリで開くための安全境界。

use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

/// フロントエンドの getCellUrl と共通の、アプリ内から開いてよい URL scheme。
/// 未登録の scheme（例: javascript:）はローカルパスとして扱い、ShellExecute に渡さない。
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

fn is_openable_url(target: &str) -> bool {
    let lower = target.to_ascii_lowercase();
    OPENABLE_URL_SCHEMES
        .iter()
        .any(|scheme| lower.starts_with(scheme))
}

fn is_file_url(target: &str) -> bool {
    target.len() >= 5 && target[..5].eq_ignore_ascii_case("file:")
}
/// 「開く」で実行を許さない拡張子。open_ref の対象文字列はデータソース側の内容
/// （CSV の 1 セル等）に由来するため、汚染されたデータで実行ファイルが起動されうる。
/// ShellExecute に渡す前にここで弾く（要件外の防御。ドキュメント・データ系は従来通り開ける）。
const DANGEROUS_OPEN_EXTS: &[&str] = &[
    "exe", "com", "scr", "pif", "bat", "cmd", "msi", "msp", "cpl", "hta", "jar", "reg", "lnk",
    "ps1", "psm1", "vbs", "vbe", "js", "jse", "wsf", "wsh", "sh", "url", "inf", "chm",
];

/// 対象パスの拡張子が実行系なら Err を返す。
fn reject_executable_target(target: &str) -> Result<(), String> {
    let ext = std::path::Path::new(target)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
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
    if is_openable_url(&target) {
        // file: URL はローカルパスと同じ安全境界に置く。クエリ/fragment は拡張子判定から除く。
        if is_file_url(&target) {
            let path_part = target.split(['?', '#']).next().unwrap_or(&target);
            reject_executable_target(path_part)?;
        }
        app.opener()
            .open_url(target, None::<&str>)
            .map_err(|error| error.to_string())
    } else {
        reject_executable_target(&target)?;
        app.opener()
            .open_path(target, None::<&str>)
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod open_ref_tests {
    use super::{is_openable_url, reject_executable_target};

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
}
