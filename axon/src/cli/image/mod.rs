mod list;

use clap::Subcommand;

pub use self::list::ListCommand;
use crate::{config::Config, error::Error};

#[derive(Clone, Subcommand)]
pub enum ImageCommands {
    #[command(alias = "l", about = "List all images")]
    List(ListCommand),
}

impl ImageCommands {
    pub async fn run(self, config: Config) -> Result<(), Error> {
        match self {
            Self::List(cmd) => cmd.run(config).await,
        }
    }
}
