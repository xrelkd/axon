mod get;
mod internal;
mod put;
mod setup;
mod shell;

use clap::Subcommand;

pub use self::{get::GetCommand, put::PutCommand, setup::SetupCommand, shell::ShellCommand};
use crate::{cli::Error, config::Config};

#[derive(Clone, Subcommand)]
pub enum SshCommands {
    #[command(about = "Set up and authorize SSH access on a temporary pod.")]
    Setup(SetupCommand),

    #[command(about = "Open an interactive SSH shell into a temporary pod.")]
    Shell(ShellCommand),

    #[command(about = "Download a file from a temporary pod via SSH.")]
    Get(GetCommand),

    #[command(about = "Upload a file to a temporary pod via SSH.")]
    Put(PutCommand),
}

impl SshCommands {
    pub async fn run(self, kube_client: kube::Client, config: Config) -> Result<(), Error> {
        match self {
            Self::Setup(cmd) => cmd.run(kube_client, config).await,
            Self::Shell(cmd) => cmd.run(kube_client, config).await,
            Self::Get(cmd) => cmd.run(kube_client, config).await,
            Self::Put(cmd) => cmd.run(kube_client, config).await,
        }
    }
}
