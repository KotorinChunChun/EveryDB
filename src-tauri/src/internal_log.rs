//! 利用者向け UI には出さない内部警告ログ（RF-60-07）。

pub(crate) fn warn(component: &str, message: impl std::fmt::Display) {
    eprintln!("[EveryDB:{component}] {message}");
}
