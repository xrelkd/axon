//! Defines the commands available under the `ssh` subcommand.
//!
//! This module groups functionalities related to SSH interactions with
//! temporary pods, including setup, interactive shell access, file upload, and
//! file download.

mod get;
mod internal;
mod put;
mod setup;
mod shell;

use clap::Subcommand;

pub use self::{get::GetCommand, put::PutCommand, setup::SetupCommand, shell::ShellCommand};
use crate::{cli::Error, config::Config};

/// Represents the various subcommands available for SSH operations.
///
/// This enum is used with `clap` to parse command-line arguments
/// and direct execution to the appropriate SSH-related task.
#[derive(Clone, Subcommand)]
pub enum SshCommands {
    /// Sets up and authorizes SSH access on a temporary pod.
    Setup(SetupCommand),

    /// Opens an interactive SSH shell into a temporary pod.
    Shell(ShellCommand),

    /// Downloads a file from a temporary pod via SSH.
    Get(GetCommand),

    /// Uploads a file to a temporary pod via SSH.
    Put(PutCommand),
}

impl SshCommands {
    /// Executes the specified SSH subcommand.
    ///
    /// This asynchronous method dispatches to the `run` method of the
    /// appropriate subcommand based on the `SshCommands` variant.
    ///
    /// # Arguments
    ///
    /// * `self` - The `SshCommands` variant representing the command to run.
    /// * `kube_client` - A Kubernetes client used to interact with the cluster.
    /// * `config` - The application's configuration.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success (`Ok(())`) or an `Error` if the command
    /// fails.
    ///
    /// # Errors
    ///
    /// This method can return an `Error` if the underlying subcommand's
    /// execution fails. Refer to the documentation of `SetupCommand::run`,
    /// `ShellCommand::run`, `GetCommand::run`, and `PutCommand::run` for
    /// specific error conditions.
    pub async fn run(self, kube_client: kube::Client, config: Config) -> Result<(), Error> {
        match self {
            Self::Setup(cmd) => cmd.run(kube_client, config).await,
            Self::Shell(cmd) => cmd.run(kube_client, config).await,
            Self::Get(cmd) => cmd.run(kube_client, config).await,
            Self::Put(cmd) => cmd.run(kube_client, config).await,
        }
    }
}
