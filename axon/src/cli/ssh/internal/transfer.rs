use std::{net::SocketAddr, path::PathBuf};

use futures::future::BoxFuture;
use tokio::io::AsyncRead;

use crate::{
    cli::{Error, ssh::internal::HandleGuard},
    ssh,
};

#[derive(Clone, Debug)]
pub enum Transfer {
    Upload { source: PathBuf, destination: PathBuf },
    Download { source: PathBuf, destination: PathBuf },
}

pub struct TransferRunner {
    pub handle: sigfinn::Handle<Error>,

    pub socket_addr: SocketAddr,

    pub ssh_private_key: russh::keys::PrivateKey,

    pub user: String,

    pub transfer: Transfer,
}

impl TransferRunner {
    pub async fn run(self) -> Result<(), Error> {
        let Self { handle, socket_addr, ssh_private_key, user, transfer } = self;

        // Automatically shuts down the port forwarder when this scope ends
        let _handle_guard = HandleGuard::from(handle);

        let session = ssh::Session::connect(ssh_private_key, user, socket_addr).await?;

        let transfer_result = match transfer {
            Transfer::Upload { source, destination } => {
                let pb = ProgressBar::new(Direction::Upload);
                let n = session
                    .upload::<_, _, _, _, _, BoxFuture<'static, ()>>(
                        source,
                        destination,
                        Some(|len| pb.set_length(len)),
                        Some(|file| pb.wrap_async_read(file)),
                        None,
                    )
                    .await;
                pb.finish();
                n
            }
            Transfer::Download { source, destination } => {
                let pb = ProgressBar::new(Direction::Download);
                let n = session
                    .download::<_, _, _, _, _, BoxFuture<'static, ()>>(
                        source,
                        destination,
                        Some(|len| pb.set_length(len)),
                        Some(|file| pb.wrap_async_read(file)),
                        None,
                    )
                    .await;
                pb.finish();
                n
            }
        };

        // Attempt to close the session cleanly
        let close_result = session.close().await;

        // Return the execution error if it exists, otherwise the closing error
        transfer_result.map(|_n| ()).map_err(Error::from)?;
        close_result.map_err(Error::from)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Direction {
    Download,
    Upload,
}

pub struct ProgressBar {
    inner: indicatif::ProgressBar,
    direction: Direction,
}

impl ProgressBar {
    pub fn new(direction: Direction) -> Self {
        let msg = match direction {
            Direction::Upload => "Uploading",
            Direction::Download => "Downloading",
        };
        let inner = indicatif::ProgressBar::new(0);
        inner.set_style(
            indicatif::ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] \
                     {bytes}/{total_bytes} ({eta}) {msg}",
                )
                .unwrap()
                .progress_chars("#>-"),
        );
        inner.set_message(msg);
        Self { inner, direction }
    }

    pub fn set_length(&self, len: u64) { self.inner.set_length(len); }

    pub fn wrap_async_read<R: AsyncRead + Unpin>(&self, read: R) -> impl AsyncRead + Unpin {
        self.inner.wrap_async_read(read)
    }

    pub fn finish(self) {
        let msg = match self.direction {
            Direction::Upload => "Upload completed",
            Direction::Download => "Download completed",
        };
        self.inner.finish_with_message(msg);
    }
}
