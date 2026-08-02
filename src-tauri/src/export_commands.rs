use crate::export_service;

#[tauri::command]
pub fn encode_shift_jis(text: String) -> Vec<u8> {
    export_service::encode_shift_jis(&text)
}
