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
    #[command(about = "Setup the SSH server in the container")]
    Setup(SetupCommand),

    #[command(about = "Connect to the SSH server in the container and open a interactive shell")]
    Shell(ShellCommand),

    #[command(about = "Get a file from container")]
    Get(GetCommand),

    #[command(about = "Put a file to container")]
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
