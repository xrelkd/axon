use clap::Args;
use snafu::ResultExt;
use tokio::io::AsyncWriteExt;

use crate::{
    cli::{Error, error},
    config::Config,
    ui::table::ImageExt,
};

#[derive(Args, Clone)]
pub struct ListCommand {}

impl ListCommand {
    pub async fn run(self, config: Config) -> Result<(), Error> {
        tokio::io::stdout()
            .write_all(config.specs.render_table().as_bytes())
            .await
            .context(error::WriteStdoutSnafu)?;
        tokio::io::stdout().write_u8(b'\n').await.context(error::WriteStdoutSnafu)
    }
}
