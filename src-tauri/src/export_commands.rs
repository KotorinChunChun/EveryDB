/// 出力ダイアログの Shift_JIS 変換用（要件: 出力の文字コード選択）。
/// ブラウザ標準の TextEncoder は Shift_JIS 等のレガシー文字コードへのエンコードに対応しないため、
/// 既存依存の encoding_rs を使ってバイト列を返す（フロントは writeFile でそのまま書き込む）。
/// 変換できない文字（機種依存文字等）は `?` に置換される（encoding_rs のエンコード仕様）。
#[tauri::command]
pub fn encode_shift_jis(text: String) -> Vec<u8> {
    let (bytes, _, _) = encoding_rs::SHIFT_JIS.encode(&text);
    bytes.into_owned()
}

#[cfg(test)]
mod export_encoding_tests {
    use super::encode_shift_jis;

    #[test]
    fn encodes_ascii_and_japanese_to_shift_jis_bytes() {
        // 「あ」の Shift_JIS は 0x82 0xA0。UTF-8 のまま書き出すと文字化けするため要確認。
        let bytes = encode_shift_jis("aあ".to_string());
        assert_eq!(bytes, vec![0x61, 0x82, 0xA0]);
    }
}
