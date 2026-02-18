mod connect;
mod copy;
mod execute;
mod setup;

use clap::Subcommand;

pub use self::{connect::ConnectCommand, setup::SetupCommand};
use crate::{config::Config, error::Error};

#[derive(Clone, Subcommand)]
pub enum SshCommands {
    #[command(about = "Setup SSH server")]
    Setup(SetupCommand),

    #[command(about = "Connect SSH server")]
    Connect(ConnectCommand),
}

impl SshCommands {
    pub async fn run(self, kube_client: kube::Client, config: Config) -> Result<(), Error> {
        match self {
            Self::Setup(cmd) => cmd.run(kube_client, config).await,
            Self::Connect(cmd) => cmd.run(kube_client, config).await,
        }
    }
}
