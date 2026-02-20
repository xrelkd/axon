use std::{net::SocketAddr, path::PathBuf};

use crate::{
    cli::{Error, ssh::internal::HandleGuard},
    ssh,
    ui::FileTransferProgressBar,
};

/// Represents the type of file transfer to be performed.
///
/// This enum distinguishes between uploading a file from a local source to a
/// remote destination and downloading a file from a remote source to a local
/// destination.
#[derive(Clone, Debug)]
pub enum FileTransfer {
    /// Specifies an upload operation.
    ///
    /// # Fields
    /// - `source`: The local path of the file to be uploaded.
    /// - `destination`: The remote path where the file will be stored.
    Upload { source: PathBuf, destination: PathBuf },
    /// Specifies a download operation.
    ///
    /// # Fields
    /// - `source`: The remote path of the file to be downloaded.
    /// - `destination`: The local path where the downloaded file will be saved.
    Download { source: PathBuf, destination: PathBuf },
}

/// A runner responsible for executing file transfer operations over an SSH
/// connection.
///
/// This struct holds all the necessary configuration and state to perform
/// either an upload or a download, including SSH credentials and connection
/// details.
pub struct FileTransferRunner {
    /// The handle to a background process (e.g., a port forwarder) that should
    /// be kept alive during the transfer and shut down afterwards.
    pub handle: sigfinn::Handle<Error>,

    /// The socket address of the remote SSH server.
    pub socket_addr: SocketAddr,

    /// The SSH private key used for authentication with the remote server.
    pub ssh_private_key: russh::keys::PrivateKey,

    /// The username for SSH authentication on the remote server.
    pub user: String,

    /// The specific file transfer operation (upload or download) to be
    /// performed.
    pub transfer: FileTransfer,
}

impl FileTransferRunner {
    /// Executes the configured file transfer operation (upload or download)
    /// over SSH.
    ///
    /// This method establishes an SSH session, performs the file transfer,
    /// and ensures proper cleanup, including the shutdown of associated
    /// resources like port forwarders. Progress bars are used to indicate
    /// transfer status.
    ///
    /// # Arguments
    ///
    /// * `shutdown_signal` - A future that, when resolved, indicates that the
    ///   transfer operation should be gracefully interrupted.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the file transfer and associated operations
    /// complete successfully. Returns `Err(Error)` if any part of the
    /// process fails.
    ///
    /// # Errors
    ///
    /// This method can return an `Error` in several scenarios:
    /// - If the SSH session cannot be established (e.g., connection failure,
    ///   authentication issues, invalid private key).
    /// - If the file upload or download operation fails (e.g., file not found,
    ///   permission denied, network issues during transfer).
    /// - If the SSH session cannot be cleanly closed after the transfer.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::{net::SocketAddr, path::PathBuf, str::FromStr};
    /// use russh::keys::key::KeyPair;
    /// use axon::{
    ///     cli::Error,
    ///     ssh,
    ///     cli::ssh::internal::HandleGuard,
    /// };
    /// use axon::cli::file_transfer_runner::{FileTransfer, FileTransferRunner};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Error> {
    ///     let shutdown_signal = tokio::signal::ctrl_c();
    ///     let private_key = KeyPair::generate_ed25519().unwrap();
    ///     let socket_addr = SocketAddr::from_str("127.0.0.1:2222").unwrap();
    ///     let user = "testuser".to_string();
    ///
    ///     // Example: Upload a file
    ///     let upload_runner = FileTransferRunner {
    ///         handle: sigfinn::Handle::new(), // In a real scenario, this would come from a running process
    ///         socket_addr,
    ///         ssh_private_key: private_key.clone(),
    ///         user: user.clone(),
    ///         transfer: FileTransfer::Upload {
    ///             source: PathBuf::from("local_file.txt"),
    ///             destination: PathBuf::from("/tmp/remote_file.txt"),
    ///         },
    ///     };
    ///
    ///     // In a real application, you would ensure the local_file.txt exists
    ///     // and the remote SSH server is running and accessible at socket_addr.
    ///     // And for a full example, handle: sigfinn::Handle::new() would be replaced
    ///     // with a handle to an actual running process.
    ///     // For demonstration purposes, we omit actual file creation and server setup.
    ///
    ///     // This call would actually execute the transfer.
    ///     // let _ = upload_runner.run(shutdown_signal.clone()).await;
    ///
    ///     // Example: Download a file
    ///     let download_runner = FileTransferRunner {
    ///         handle: sigfinn::Handle::new(), // In a real scenario, this would come from a running process
    ///         socket_addr,
    ///         ssh_private_key: private_key,
    ///         user,
    ///         transfer: FileTransfer::Download {
    ///             source: PathBuf::from("/tmp/remote_file.txt"),
    ///             destination: PathBuf::from("downloaded_file.txt"),
    ///         },
    ///     };
    ///
    ///     // Again, in a real application, ensure the remote_file.txt exists on the server.
    ///     // let _ = download_runner.run(shutdown_signal).await;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn run(self, shutdown_signal: impl Future<Output = ()> + Unpin) -> Result<(), Error> {
        let Self { handle, socket_addr, ssh_private_key, user, transfer } = self;

        // Automatically shuts down the port forwarder when this scope ends
        let _handle_guard = HandleGuard::from(handle);

        let session = ssh::Session::connect(ssh_private_key, user, socket_addr).await?;

        let transfer_result = match transfer {
            FileTransfer::Upload { source, destination } => {
                let pb = FileTransferProgressBar::new_upload();
                let n = session
                    .upload(
                        source,
                        destination,
                        Some(|len| pb.set_length(len)),
                        Some(|file| pb.wrap_async_read(file)),
                        Some(shutdown_signal),
                    )
                    .await;
                if n.is_ok() {
                    pb.finish();
                }
                n
            }
            FileTransfer::Download { source, destination } => {
                let pb = FileTransferProgressBar::new_download();
                let n = session
                    .download(
                        source,
                        destination,
                        Some(|len| pb.set_length(len)),
                        Some(|file| pb.wrap_async_read(file)),
                        Some(shutdown_signal),
                    )
                    .await;
                if n.is_ok() {
                    pb.finish();
                }
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
