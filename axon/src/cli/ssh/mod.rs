mod copy;
mod setup;
mod shell;

use clap::Subcommand;

pub use self::{setup::SetupCommand, shell::ShellCommand};
use crate::{config::Config, error::Error};

#[derive(Clone, Subcommand)]
pub enum SshCommands {
    #[command(about = "Setup the SSH server in the container")]
    Setup(SetupCommand),

    #[command(about = "Connect to the SSH server in the container and open a interactive shell")]
    Shell(ShellCommand),
}

impl SshCommands {
    pub async fn run(self, kube_client: kube::Client, config: Config) -> Result<(), Error> {
        match self {
            Self::Setup(cmd) => cmd.run(kube_client, config).await,
            Self::Shell(cmd) => cmd.run(kube_client, config).await,
        }
    }
}
