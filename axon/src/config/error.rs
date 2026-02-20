use std::path::PathBuf;

use snafu::Snafu;

/// Represents the possible errors that can occur when handling configuration
/// files.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    /// Error returned when the configuration file specified by `filename`
    /// fails to open.
    ///
    /// # Arguments
    ///
    /// * `filename` - The path to the configuration file that failed to open.
    /// * `source` - The underlying [`std::io::Error`] that occurred.
    #[snafu(display("Failed to open config from {}, error: {source}", filename.display()))]
    OpenConfig { filename: PathBuf, source: std::io::Error },

    /// Error returned when the content of the configuration file specified by
    /// `filename` fails to be parsed (e.g., due to invalid YAML syntax).
    ///
    /// # Arguments
    ///
    /// * `filename` - The path to the configuration file that failed to parse.
    /// * `source` - The underlying [`serde_yaml::Error`] that occurred during
    ///   parsing.
    #[snafu(display("Failed to parse config from {}, error: {source}", filename.display()))]
    ParseConfig { filename: PathBuf, source: serde_yaml::Error },

    /// Error returned when a file path cannot be resolved to its canonical
    /// form. This might happen if the path does not exist or if there are
    /// insufficient permissions to access it.
    ///
    /// # Arguments
    ///
    /// * `file_path` - The path that could not be resolved.
    /// * `source` - The underlying [`std::io::Error`] that occurred during path
    ///   resolution.
    #[snafu(display("Failed to resolve file path {}, error: {source}", file_path.display()))]
    ResolveFilePath { file_path: PathBuf, source: std::io::Error },
}
