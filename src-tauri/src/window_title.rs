/// KotorinToys共通規約に従ったメインウィンドウのタイトル。
pub(crate) struct WindowTitle;

impl WindowTitle {
    pub(crate) fn create() -> String {
        format!("EveryDB - ver {}", env!("CARGO_PKG_VERSION"))
    }
}

#[cfg(test)]
mod tests {
    use super::WindowTitle;

    #[test]
    fn main_window_title_uses_the_package_version() {
        assert_eq!(
            WindowTitle::create(),
            format!("EveryDB - ver {}", env!("CARGO_PKG_VERSION"))
        );
    }
}
