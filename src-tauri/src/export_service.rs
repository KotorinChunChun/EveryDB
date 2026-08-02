//! エクスポート用の文字列変換。Tauri 型へ依存しない。

/// Shift_JIS バイト列へ変換する（変換不能文字は `?`）。
pub fn encode_shift_jis(text: &str) -> Vec<u8> {
    let (bytes, _, _) = encoding_rs::SHIFT_JIS.encode(text);
    bytes.into_owned()
}

#[cfg(test)]
mod tests {
    use super::encode_shift_jis;

    #[test]
    fn encodes_ascii_and_japanese_to_shift_jis_bytes() {
        let bytes = encode_shift_jis("aあ");
        assert_eq!(bytes, vec![0x61, 0x82, 0xA0]);
    }
}
