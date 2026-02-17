use std::path::{Path, PathBuf};

use directories::BaseDirs;

/// # Panics
/// This function should never panic
#[inline]
pub fn default_unix_domain_socket() -> PathBuf {
    let base_dirs = BaseDirs::new().expect("`BaseDirs::new` always success");
    [
        base_dirs.runtime_dir().map_or_else(std::env::temp_dir, Path::to_path_buf),
        PathBuf::from(crate::PROJECT_NAME),
        PathBuf::from("grpc.sock"),
    ]
    .into_iter()
    .collect()
}
