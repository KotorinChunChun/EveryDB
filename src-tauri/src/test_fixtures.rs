//! 通常テストで使用する架空データの参照先。製品には含めない。
use std::path::{Path, PathBuf};

pub(crate) fn sample(name: &str) -> PathBuf {
    std::env::var_os("EVERYDB_TEST_SAMPLES_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("samples")
        })
        .join(name)
}
