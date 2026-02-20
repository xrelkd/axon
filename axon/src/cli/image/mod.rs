//! Defines the commands for managing container images within the CLI.

mod list;

use clap::Subcommand;

pub use self::list::ListCommand;
use crate::{cli::Error, config::Config};

/// Represents the available subcommands for image-related operations.
///
/// These commands allow users to interact with predefined container image
/// specifications as configured in the application.
#[derive(Clone, Subcommand)]
pub enum ImageCommands {
    /// Lists all predefined container image specifications in the application's
    /// configuration.
    ///
    /// This command provides an overview of the available image configurations,
    /// including their names and potentially other relevant details.
    #[command(
        alias = "l",
        about = "List all predefined container image specifications in the configuration."
    )]
    List(ListCommand),
}

impl ImageCommands {
    /// Executes the specified image command.
    ///
    /// This asynchronous function dispatches to the appropriate handler based
    /// on the `ImageCommands` variant.
    ///
    /// # Arguments
    ///
    /// * `self` - The `ImageCommands` variant representing the command to be
    ///   executed.
    /// * `config` - The application's configuration, containing necessary
    ///   settings and predefined image specifications.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] if the underlying command (e.g.,
    /// `ListCommand::run`) encounters an issue during execution.
    pub async fn run(self, config: Config) -> Result<(), Error> {
        match self {
            Self::List(cmd) => cmd.run(config).await,
        }
    }
}
