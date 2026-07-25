//! 秘密情報の保護（要件 §4-3-bis）と、マスターパスワードによる閲覧ゲート（要件 §9-4）。
//! Windows では DPAPI（現在ユーザーに紐づく暗号化）で秘密フィールドを暗号化して保存する。
//! config_json 内の秘密フィールドのみを `dpapi:<hex>` 形式に置換する。

use anyhow::{anyhow, Result};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use serde_json::Value;

/// マスターパスワードを argon2 でハッシュ化する（DB へは元パスワードを保存しない）。
pub fn hash_master_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow!("パスワードのハッシュ化に失敗: {e}"))?;
    Ok(hash.to_string())
}

/// 入力パスワードが保存済みハッシュと一致するか検証する。
pub fn verify_master_password(password: &str, stored_hash: &str) -> Result<bool> {
    let parsed = PasswordHash::new(stored_hash).map_err(|e| anyhow!("保存済みハッシュの解析に失敗: {e}"))?;
    Ok(Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
}

const MARKER: &str = "dpapi:";
/// 編集ダイアログでロック中に秘密フィールドの代わりに表示するプレースホルダー。
/// 保存時、この値のままなら「変更なし」とみなし旧暗号化値を維持する。
pub const SECRET_PLACEHOLDER: &str = "••••••••";

/// この種別が秘密フィールドを持つか（マスターパスワード必須化の判定に使用。要件 §9-4）。
pub fn has_secret_fields(type_: &str) -> bool {
    !secret_keys(type_).is_empty()
}

/// 種別ごとの秘密フィールド名（ここに列挙したキーだけ暗号化対象）。
fn secret_keys(type_: &str) -> &'static [&'static str] {
    match type_ {
        "notion" => &["token"],
        "airtable" => &["token"],
        "postgres" | "mysql" => &["password"],
        "box_files" => &["client_secret"],
        // Graph 系（graph_entra_users 等）はすべて client_secret を持つ。
        t if t.starts_with("graph") => &["client_secret"],
        _ => &[],
    }
}

/// 保存前：種別の秘密フィールドを暗号化する。
pub fn protect_config(type_: &str, config_json: &str) -> Result<String> {
    let keys = secret_keys(type_);
    if keys.is_empty() {
        return Ok(config_json.to_string());
    }
    let mut v: Value = serde_json::from_str(config_json)?;
    if let Some(obj) = v.as_object_mut() {
        for k in keys {
            let cur = obj.get(*k).and_then(|x| x.as_str()).map(|s| s.to_string());
            if let Some(s) = cur {
                if !s.is_empty() && !s.starts_with(MARKER) {
                    let enc = format!("{MARKER}{}", to_hex(&dpapi_protect(s.as_bytes())?));
                    obj.insert((*k).to_string(), Value::String(enc));
                }
            }
        }
    }
    Ok(serde_json::to_string(&v)?)
}

/// 利用時：`dpapi:` 付きの文字列値をすべて復号する。
pub fn unprotect_config(config_json: &str) -> Result<String> {
    let mut v: Value = serde_json::from_str(config_json)?;
    if let Some(obj) = v.as_object_mut() {
        let keys: Vec<String> = obj.keys().cloned().collect();
        for k in keys {
            let cur = obj.get(&k).and_then(|x| x.as_str()).map(|s| s.to_string());
            if let Some(s) = cur {
                if let Some(hex) = s.strip_prefix(MARKER) {
                    let dec = String::from_utf8(dpapi_unprotect(&from_hex(hex)?)?)?;
                    obj.insert(k, Value::String(dec));
                }
            }
        }
    }
    Ok(serde_json::to_string(&v)?)
}

/// マスターパスワード未解錠時、編集ダイアログに渡す用に秘密フィールドをプレースホルダーへ置換する。
pub fn mask_config(config_json: &str) -> Result<String> {
    let mut v: Value = serde_json::from_str(config_json)?;
    if let Some(obj) = v.as_object_mut() {
        for (_, val) in obj.iter_mut() {
            if val.as_str().map(|s| s.starts_with(MARKER)).unwrap_or(false) {
                *val = Value::String(SECRET_PLACEHOLDER.to_string());
            }
        }
    }
    Ok(serde_json::to_string(&v)?)
}

/// 編集保存時：新しい config でプレースホルダーのままの秘密フィールドは、
/// 旧設定の暗号化済み値をそのまま引き継ぐ（＝ユーザーが触っていない = 変更なし扱い）。
pub fn merge_preserving_placeholder(type_: &str, old_config_json: &str, new_config_json: &str) -> Result<String> {
    let keys = secret_keys(type_);
    if keys.is_empty() {
        return Ok(new_config_json.to_string());
    }
    let old: Value = serde_json::from_str(old_config_json)?;
    let mut new_v: Value = serde_json::from_str(new_config_json)?;
    if let Some(new_obj) = new_v.as_object_mut() {
        for k in keys {
            if new_obj.get(*k).and_then(|v| v.as_str()) == Some(SECRET_PLACEHOLDER) {
                if let Some(old_val) = old.get(*k) {
                    new_obj.insert((*k).to_string(), old_val.clone());
                }
            }
        }
    }
    Ok(serde_json::to_string(&new_v)?)
}

fn to_hex(b: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(b.len() * 2);
    for x in b {
        let _ = write!(s, "{x:02x}");
    }
    s
}

fn from_hex(h: &str) -> Result<Vec<u8>> {
    if h.len() % 2 != 0 {
        return Err(anyhow!("不正な hex 長"));
    }
    (0..h.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&h[i..i + 2], 16).map_err(|e| anyhow!("hex 復号失敗: {e}")))
        .collect()
}

#[cfg(windows)]
fn dpapi_protect(plain: &[u8]) -> Result<Vec<u8>> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{CryptProtectData, CRYPT_INTEGER_BLOB};
    unsafe {
        let input = CRYPT_INTEGER_BLOB { cbData: plain.len() as u32, pbData: plain.as_ptr() as *mut u8 };
        let mut out = CRYPT_INTEGER_BLOB::default();
        CryptProtectData(&input, PCWSTR::null(), None, None, None, 0, &mut out)
            .map_err(|e| anyhow!("CryptProtectData 失敗: {e}"))?;
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(out.pbData as *mut core::ffi::c_void)));
        Ok(v)
    }
}

#[cfg(windows)]
fn dpapi_unprotect(cipher: &[u8]) -> Result<Vec<u8>> {
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{CryptUnprotectData, CRYPT_INTEGER_BLOB};
    unsafe {
        let input = CRYPT_INTEGER_BLOB { cbData: cipher.len() as u32, pbData: cipher.as_ptr() as *mut u8 };
        let mut out = CRYPT_INTEGER_BLOB::default();
        CryptUnprotectData(&input, None, None, None, None, 0, &mut out)
            .map_err(|e| anyhow!("CryptUnprotectData 失敗（別ユーザー/別マシンの暗号文の可能性）: {e}"))?;
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(out.pbData as *mut core::ffi::c_void)));
        Ok(v)
    }
}

// 非 Windows（CI 等）では素通し。実運用は Windows 前提。
#[cfg(not(windows))]
fn dpapi_protect(plain: &[u8]) -> Result<Vec<u8>> {
    Ok(plain.to_vec())
}
#[cfg(not(windows))]
fn dpapi_unprotect(cipher: &[u8]) -> Result<Vec<u8>> {
    Ok(cipher.to_vec())
}

#[cfg(test)]
mod master_password_tests {
    use super::*;

    #[test]
    fn hash_and_verify_roundtrip() {
        let hash = hash_master_password("correct horse battery staple").unwrap();
        assert!(hash.starts_with("$argon2"));
        assert!(verify_master_password("correct horse battery staple", &hash).unwrap());
        assert!(!verify_master_password("wrong password", &hash).unwrap());
    }

    #[test]
    fn mask_replaces_only_secret_looking_values() {
        let cfg = serde_json::json!({ "token": "dpapi:aabb", "database_id": "abc" }).to_string();
        let masked = mask_config(&cfg).unwrap();
        let v: Value = serde_json::from_str(&masked).unwrap();
        assert_eq!(v["token"], SECRET_PLACEHOLDER);
        assert_eq!(v["database_id"], "abc"); // dpapi: で始まらない値はそのまま
    }

    #[test]
    fn merge_preserves_old_secret_when_placeholder_untouched() {
        let old = serde_json::json!({ "token": "dpapi:aabbcc", "database_id": "old_db" }).to_string();
        let new = serde_json::json!({ "token": SECRET_PLACEHOLDER, "database_id": "new_db" }).to_string();
        let merged = merge_preserving_placeholder("notion", &old, &new).unwrap();
        let v: Value = serde_json::from_str(&merged).unwrap();
        assert_eq!(v["token"], "dpapi:aabbcc"); // プレースホルダーのままなら旧暗号化値を維持
        assert_eq!(v["database_id"], "new_db"); // 非秘密フィールドは新しい値を採用
    }

    #[test]
    fn merge_adopts_new_secret_when_actually_changed() {
        let old = serde_json::json!({ "token": "dpapi:aabbcc" }).to_string();
        let new = serde_json::json!({ "token": "ntn_brand_new_value" }).to_string();
        let merged = merge_preserving_placeholder("notion", &old, &new).unwrap();
        let v: Value = serde_json::from_str(&merged).unwrap();
        assert_eq!(v["token"], "ntn_brand_new_value"); // プレースホルダーでなければ新しい値を採用
    }

    #[test]
    fn same_password_hashes_differently_each_time() {
        // ソルトが毎回ランダムなので、同じパスワードでもハッシュ文字列は一致しない。
        let h1 = hash_master_password("same").unwrap();
        let h2 = hash_master_password("same").unwrap();
        assert_ne!(h1, h2);
        assert!(verify_master_password("same", &h1).unwrap());
        assert!(verify_master_password("same", &h2).unwrap());
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn protect_then_unprotect_roundtrip() {
        let cfg = serde_json::json!({ "token": "ntn_secret_value_123", "database_id": "abc" }).to_string();
        let prot = protect_config("notion", &cfg).unwrap();
        // 暗号化されて平文トークンが消え、マーカーが付く。
        assert!(prot.contains("dpapi:"));
        assert!(!prot.contains("ntn_secret_value_123"));
        // 非秘密フィールドはそのまま。
        assert!(prot.contains("\"database_id\":\"abc\""));

        let unprot = unprotect_config(&prot).unwrap();
        let v: Value = serde_json::from_str(&unprot).unwrap();
        assert_eq!(v["token"], "ntn_secret_value_123");
        assert_eq!(v["database_id"], "abc");
    }

    #[test]
    fn non_secret_type_is_untouched() {
        let cfg = serde_json::json!({ "path": "C:\\a.csv", "has_header": true }).to_string();
        let prot = protect_config("csv", &cfg).unwrap();
        assert_eq!(prot, cfg); // csv には秘密フィールドが無いので不変
    }

    #[test]
    fn double_protect_is_idempotent_on_marker() {
        let cfg = serde_json::json!({ "token": "abc" }).to_string();
        let once = protect_config("notion", &cfg).unwrap();
        let twice = protect_config("notion", &once).unwrap();
        assert_eq!(once, twice); // 既に dpapi: 付きなら二重暗号化しない
    }
}
