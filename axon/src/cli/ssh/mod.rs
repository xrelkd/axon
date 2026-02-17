mod setup;

use clap::Subcommand;

pub use self::setup::SetupCommand;
use crate::{config::Config, error::Error};

#[derive(Clone, Subcommand)]
pub enum SshCommands {
    #[command(about = "Setup SSH")]
    Setup(SetupCommand),
}

impl SshCommands {
    pub async fn run(self, kube_client: kube::Client, config: Config) -> Result<(), Error> {
        match self {
            Self::Setup(cmd) => cmd.run(kube_client, config).await,
        }
    }
}
