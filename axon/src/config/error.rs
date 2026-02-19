use std::path::PathBuf;

use snafu::Snafu;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    #[snafu(display("Failed to open config from {}, error: {source}", filename.display()))]
    OpenConfig { filename: PathBuf, source: std::io::Error },

    #[snafu(display("Failed to parse config from {}, error: {source}", filename.display()))]
    ParseConfig { filename: PathBuf, source: serde_yaml::Error },

    #[snafu(display("Failed to resolve file path {}, error: {source}", file_path.display()))]
    ResolveFilePath { file_path: PathBuf, source: std::io::Error },
}
