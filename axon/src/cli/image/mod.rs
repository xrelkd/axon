mod list;

use clap::Subcommand;

use crate::{cli::image::list::ListCommand, config::Config, error::Error};

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
