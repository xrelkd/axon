use clap::Args;
use snafu::ResultExt;
use tokio::io::AsyncWriteExt;

use crate::{
    cli::{Error, error},
    config::Config,
    ui::table::SpecExt,
};

/// Represents the `list` subcommand for the CLI.
///
/// This struct holds no specific arguments itself, but acts as a marker
/// for the `list` operation, which displays configured specifications.
#[derive(Args, Clone)]
pub struct ListCommand {}

impl ListCommand {
    /// Executes the `list` command, printing all configured specifications to
    /// standard output.
    ///
    /// It formats the specifications as a table and writes them to stdout,
    /// followed by a newline character.
    ///
    /// # Arguments
    ///
    /// * `self` - The `ListCommand` instance.
    /// * `config` - The application's configuration, containing the
    ///   specifications to be listed.
    ///
    /// # Errors
    ///
    /// This function will return an `Error` if it fails to write to standard
    /// output.
    pub async fn run(self, config: Config) -> Result<(), Error> {
        tokio::io::stdout()
            .write_all(config.specs.render_table().as_bytes())
            .await
            .context(error::WriteStdoutSnafu)?;
        tokio::io::stdout().write_u8(b'\n').await.context(error::WriteStdoutSnafu)
    }
}
