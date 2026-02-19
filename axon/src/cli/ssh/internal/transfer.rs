use std::{net::SocketAddr, path::PathBuf};

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
            Transfer::Upload { source, destination } => session.upload(source, destination).await,
            Transfer::Download { source, destination } => {
                session.download(source, destination).await
            }
        };

        // Attempt to close the session cleanly
        let close_result = session.close().await;

        // Return the execution error if it exists, otherwise the closing error
        transfer_result.map(|_n| ()).map_err(Error::from)?;
        close_result.map_err(Error::from)
    }
}
