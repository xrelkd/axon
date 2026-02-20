//! This module provides an SSH client session for connecting to a remote host,
//! executing commands, and performing file transfers (upload/download) over
//! SFTP.

use std::{path::Path, sync::Arc, time::Duration};

use futures::{FutureExt, future};
use russh::{
    ChannelMsg, Disconnect, client,
    keys::{PrivateKey, PublicKey, key::PrivateKeyWithHashAlg},
};
use russh_sftp::{client::SftpSession, protocol::OpenFlags};
use snafu::{IntoError, ResultExt};
use tokio::{
    fs::File as LocalFile,
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    net::ToSocketAddrs,
};
use tokio_util::either::Either as AsyncEither;

use crate::ssh::{error, error::Error};

/// A client handler for `russh` sessions.
///
/// This struct implements the `client::Handler` trait, primarily to handle
/// server key verification.
#[derive(Default)]
struct Client {}

impl client::Handler for Client {
    type Error = russh::Error;

    /// Checks the server's public key during the SSH handshake.
    ///
    /// This implementation currently accepts any server key, which is suitable
    /// for scenarios where host key checking is managed externally or
    /// during development.
    ///
    /// # Arguments
    ///
    /// * `_server_public_key` - The public key presented by the server.
    ///
    /// # Returns
    ///
    /// `Ok(true)` always, indicating the server key is accepted.
    async fn check_server_key(
        &mut self,
        _server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

/// Represents an active SSH session to a remote host.
///
/// This session can be used to execute commands and perform SFTP operations.
pub struct Session {
    session: client::Handle<Client>,
}

impl Session {
    /// Establishes a new SSH session to a remote host using public key
    /// authentication.
    ///
    /// # Arguments
    ///
    /// * `private_key` - The private key used for authentication.
    /// * `user` - The username for authentication on the remote host.
    /// * `addrs` - The address of the remote host (e.g., "localhost:22",
    ///   "192.168.1.1:22").
    ///
    /// # Errors
    ///
    /// This function returns an `Error` if:
    /// - The connection to the server fails (`error::ConnectServerSnafu`).
    /// - The public key authentication fails (`error::AuthenticateUserSnafu`).
    /// - Access is denied after successful authentication
    ///   (`error::DenyAccessSnafu`).
    ///
    /// # Returns
    ///
    /// A `Result` containing the established `Session` on success, or an
    /// `Error` on failure.
    ///
    /// # Example
    /// ```no_run
    /// use std::path::Path;
    /// use russh::keys::PrivateKey;
    /// use crate::ssh::{session::Session, error};
    /// use snafu::ResultExt;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     // This example assumes a 'ReadPrivateKeySnafu' error variant exists
    ///     // in your error module for reading the private key.
    ///     // Replace "/path/to/id_rsa" with the actual path to your private key.
    ///     let private_key_path = Path::new("id_rsa");
    ///     let private_key = PrivateKey::read_pkcs8(private_key_path, None)
    ///         .await
    ///         .context(error::ReadPrivateKeySnafu)?;
    ///
    ///     let session = Session::connect(private_key, "user", "localhost:22")
    ///         .await?;
    ///
    ///     println!("SSH session established!");
    ///     session.close().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn connect<A: ToSocketAddrs>(
        private_key: PrivateKey,
        user: impl Into<String>,
        addrs: A,
    ) -> Result<Self, Error> {
        let mut session = {
            let client = Client::default();
            let config = Arc::new(client::Config {
                inactivity_timeout: Some(Duration::from_secs(5)),
                ..<_>::default()
            });
            client::connect(config, addrs, client).await.context(error::ConnectServerSnafu)?
        };

        let best_hash =
            session.best_supported_rsa_hash().await.context(error::ConnectServerSnafu)?.flatten();

        let user_str = user.into();
        let auth_res = session
            .authenticate_publickey(
                &user_str,
                PrivateKeyWithHashAlg::new(Arc::new(private_key), best_hash),
            )
            .await
            .with_context(|_| error::AuthenticateUserSnafu { user: user_str.clone() })?;

        snafu::ensure!(auth_res.success(), error::DenyAccessSnafu { user: user_str.clone() });

        Ok(Self { session })
    }

    /// Executes a command on the remote host and streams stdin/stdout.
    ///
    /// This function sets up a pseudo-terminal (PTY), executes the given
    /// command, and pipes standard input/output between the local and
    /// remote processes. It blocks until the remote command completes.
    ///
    /// # Arguments
    ///
    /// * `command` - The command string to execute on the remote host.
    ///
    /// # Errors
    ///
    /// This function returns an `Error` if:
    /// - Opening a new channel fails (`error::OpenChannelSnafu`).
    /// - Retrieving terminal size fails (`error::GetTerminalSizeSnafu`).
    /// - Requesting a pseudo-terminal (PTY) fails (`error::RequestPtySnafu`).
    /// - Executing the command fails (`error::ExecuteCommandSnafu`).
    /// - Initializing standard I/O for stdin/stdout fails
    ///   (`error::InitializeStdioSnafu`).
    /// - Reading from local stdin fails (`error::ReadStdinSnafu`).
    /// - Sending data to the remote channel fails
    ///   (`error::SendChannelDataSnafu`).
    /// - Writing to local stdout fails (`error::WriteStdoutSnafu`).
    /// - Closing the channel fails (`error::CloseChannelSnafu`).
    ///
    /// # Returns
    ///
    /// A `Result` containing the exit status code of the remote command on
    /// success, or an `Error` on failure.
    ///
    /// # Example
    /// ```no_run
    /// use std::path::Path;
    /// use russh::keys::PrivateKey;
    /// use crate::ssh::{session::Session, error};
    /// use snafu::ResultExt;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     // This example assumes a 'ReadPrivateKeySnafu' error variant exists
    ///     // in your error module for reading the private key.
    ///     // Replace "/path/to/id_rsa" with the actual path to your private key.
    ///     let private_key_path = Path::new("id_rsa");
    ///     let private_key = PrivateKey::read_pkcs8(private_key_path, None)
    ///         .await
    ///         .context(error::ReadPrivateKeySnafu)?;
    ///
    ///     let session = Session::connect(private_key, "user", "localhost:22")
    ///         .await?;
    ///
    ///     println!("Executing 'echo Hello, remote world!' on remote...");
    ///     let exit_code = session.call("echo Hello, remote world!").await?;
    ///     println!("Command finished with exit code: {}", exit_code);
    ///
    ///     session.close().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn call(&self, command: &str) -> Result<u32, Error> {
        let mut channel =
            self.session.channel_open_session().await.context(error::OpenChannelSnafu)?;

        let term = std::env::var("TERM").unwrap_or_else(|_| "xterm".into());
        let (width, height) = crossterm::terminal::size().context(error::GetTerminalSizeSnafu)?;
        channel
            .request_pty(false, &term, u32::from(width), u32::from(height), 0, 0, &[])
            .await
            .context(error::RequestPtySnafu)?;
        channel.exec(true, command).await.context(error::ExecuteCommandSnafu)?;

        let code;
        let mut stdin = tokio_fd::AsyncFd::try_from(0)
            .context(error::InitializeStdioSnafu { stream: "stdin" })?;
        let mut stdout = tokio_fd::AsyncFd::try_from(1)
            .context(error::InitializeStdioSnafu { stream: "stdout" })?;
        let mut buf = vec![0; 4096];
        let mut stdin_closed = false;

        loop {
            tokio::select! {
                r = stdin.read(&mut buf), if !stdin_closed => {
                    match r {
                        Ok(0) => {
                            stdin_closed = true;
                            channel.eof().await.context(error::CloseChannelSnafu)?;
                        },
                        Ok(n) => channel.data(&buf[..n]).await.context(error::SendChannelDataSnafu)?,
                        Err(source) => return Err(error::ReadStdinSnafu.into_error(source)),
                    }
                },
                Some(msg) = channel.wait() => {
                    match msg {
                        ChannelMsg::Data { ref data } => {
                            stdout.write_all(data).await.context(error::WriteStdoutSnafu)?;
                            stdout.flush().await.context(error::WriteStdoutSnafu)?;
                        }
                        ChannelMsg::ExitStatus { exit_status } => {
                            code = exit_status;
                            if !stdin_closed {
                                channel.eof().await.context(error::CloseChannelSnafu)?;
                            }
                            break;
                        }
                        _ => {}
                    }
                },
            }
        }
        Ok(code)
    }

    /// Uploads a local file to the remote host via SFTP.
    ///
    /// # Arguments
    ///
    /// * `src` - The path to the local file to upload.
    /// * `dst` - The destination path on the remote host.
    /// * `on_length` - An optional closure that will be called with the total
    ///   length of the file once it's known. Useful for progress indicators.
    /// * `reader_wrapper` - An optional function to wrap the `tokio::fs::File`
    ///   reader, allowing for custom processing or progress tracking during the
    ///   read.
    /// * `cancel_signal` - An optional future that, if resolved, will cancel
    ///   the upload operation.
    ///
    /// # Errors
    ///
    /// This function returns an `Error` if:
    /// - The local source file cannot be opened or its metadata accessed
    ///   (`error::OpenLocalFileSnafu`).
    /// - The SFTP session cannot be prepared (errors from
    ///   `prepare_sftp_session`).
    /// - The remote destination file cannot be opened or created
    ///   (`Error::OpenRemoteFile`).
    /// - Data transfer between local and remote fails
    ///   (`error::TransferDataSnafu`).
    /// - The upload operation is cancelled by the `cancel_signal`
    ///   (`Error::Cancelled`).
    ///
    /// # Returns
    ///
    /// A `Result` containing the number of bytes uploaded on success, or an
    /// `Error` on failure.
    ///
    /// # Example
    /// ```no_run
    /// use std::path::Path;
    /// use russh::keys::PrivateKey;
    /// use crate::ssh::{session::Session, error};
    /// use snafu::ResultExt;
    /// use tokio::sync::oneshot;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let private_key_path = Path::new("id_rsa");
    ///     let private_key = PrivateKey::read_pkcs8(private_key_path, None)
    ///         .await
    ///         .context(error::ReadPrivateKeySnafu)?;
    ///
    ///     let session = Session::connect(private_key, "user", "localhost:22")
    ///         .await?;
    ///
    ///     let local_path = Path::new("local_file_to_upload.txt");
    ///     let remote_path = Path::new("/tmp/remote_file_uploaded.txt");
    ///
    ///     // Create a dummy local file for the example
    ///     tokio::fs::write(&local_path, "Hello, SFTP upload from local!").await?;
    ///
    ///     let (cancel_tx, cancel_rx) = oneshot::channel();
    ///
    ///     println!("Uploading {} to {}", local_path.display(), remote_path.display());
    ///     let uploaded_bytes = session.upload(
    ///         &local_path,
    ///         &remote_path,
    ///         Some(|len| println!("File size: {} bytes", len)),
    ///         None::<fn(tokio::fs::File) -> tokio::fs::File>, // No custom wrapper
    ///         Some(cancel_rx.map(|_| ())), // Convert oneshot::Receiver into a Future<Output=()>
    ///     ).await?;
    ///
    ///     println!("Successfully uploaded {} bytes.", uploaded_bytes);
    ///
    ///     // Clean up dummy file
    ///     tokio::fs::remove_file(&local_path).await?;
    ///
    ///     session.close().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn upload<S, D, L, R, F, Sig>(
        &self,
        src: S,
        dst: D,
        on_length: Option<L>,
        reader_wrapper: Option<F>,
        cancel_signal: Option<Sig>,
    ) -> Result<u64, Error>
    where
        S: AsRef<Path>,
        D: AsRef<Path>,
        L: FnOnce(u64),
        R: AsyncRead + Send + Unpin,
        F: FnOnce(LocalFile) -> R,
        Sig: Future<Output = ()> + Unpin,
    {
        let src = src.as_ref();
        let dst = dst.as_ref();

        let local_file =
            LocalFile::open(src).await.context(error::OpenLocalFileSnafu { path: src })?;

        if let Some(on_length) = on_length {
            let _unused = local_file
                .metadata()
                .await
                .inspect(|metadata| {
                    on_length(metadata.len());
                })
                .context(error::OpenLocalFileSnafu { path: src })?;
        }

        let dst_str = dst.to_string_lossy().to_string();
        let sftp = self.prepare_sftp_session().await?;

        let mut remote_file = sftp
            .open_with_flags(&dst_str, OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE)
            .await
            .map_err(|source| Error::OpenRemoteFile { path: dst_str, source })?;

        // Wrap reader if provided
        let mut local_file = match reader_wrapper {
            Some(wrapper) => AsyncEither::Left(wrapper(local_file)),
            None => AsyncEither::Right(local_file),
        };

        // Create the copy future
        let copy_task = tokio::io::copy(&mut local_file, &mut remote_file).boxed();

        let n = match cancel_signal {
            Some(sig) => match future::select(copy_task, sig).await {
                future::Either::Left((copy_res, _)) => {
                    copy_res.context(error::TransferDataSnafu { path: src })?
                }
                future::Either::Right((..)) => return Err(Error::Cancelled),
            },
            None => copy_task.await.context(error::TransferDataSnafu { path: src })?,
        };

        let _ = remote_file.shutdown().await.ok();
        Ok(n)
    }

    /// Downloads a remote file from the host via SFTP to a local destination.
    ///
    /// # Arguments
    ///
    /// * `src` - The path to the remote file to download.
    /// * `dst` - The destination path for the local file.
    /// * `on_length` - An optional closure that will be called with the total
    ///   length of the file once it's known. Useful for progress indicators.
    /// * `reader_wrapper` - An optional function to wrap the
    ///   `russh_sftp::client::fs::File` reader, allowing for custom processing
    ///   or progress tracking during the read.
    /// * `cancel_signal` - An optional future that, if resolved, will cancel
    ///   the download operation.
    ///
    /// # Errors
    ///
    /// This function returns an `Error` if:
    /// - The SFTP session cannot be prepared (errors from
    ///   `prepare_sftp_session`).
    /// - The remote source file cannot be opened or its metadata accessed
    ///   (`error::OpenRemoteFileSnafu`).
    /// - The local destination file cannot be created
    ///   (`error::OpenLocalFileSnafu`).
    /// - Data transfer between remote and local fails
    ///   (`error::TransferDataSnafu`).
    /// - The download operation is cancelled by the `cancel_signal`
    ///   (`Error::Cancelled`).
    ///
    /// # Returns
    ///
    /// A `Result` containing the number of bytes downloaded on success, or an
    /// `Error` on failure.
    ///
    /// # Example
    /// ```no_run
    /// use std::path::Path;
    /// use russh::keys::PrivateKey;
    /// use crate::ssh::{session::Session, error};
    /// use snafu::ResultExt;
    /// use tokio::sync::oneshot;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let private_key_path = Path::new("id_rsa");
    ///     let private_key = PrivateKey::read_pkcs8(private_key_path, None)
    ///         .await
    ///         .context(error::ReadPrivateKeySnafu)?;
    ///
    ///     let session = Session::connect(private_key, "user", "localhost:22")
    ///         .await?;
    ///
    ///     let remote_path = Path::new("/tmp/remote_file_to_download.txt");
    ///     let local_path = Path::new("downloaded_remote_file.txt");
    ///
    ///     // For a real example, ensure the remote_path exists on the server.
    ///     // Here we assume it does or a prior upload created it.
    ///
    ///     let (cancel_tx, cancel_rx) = oneshot::channel();
    ///
    ///     println!("Downloading {} to {}", remote_path.display(), local_path.display());
    ///     let downloaded_bytes = session.download(
    ///         &remote_path,
    ///         &local_path,
    ///         Some(|len| println!("File size: {} bytes", len)),
    ///         None::<fn(russh_sftp::client::fs::File) -> russh_sftp::client::fs::File>, // No custom wrapper
    ///         Some(cancel_rx.map(|_| ())), // Convert oneshot::Receiver into a Future<Output=()>
    ///     ).await?;
    ///
    ///     println!("Successfully downloaded {} bytes.", downloaded_bytes);
    ///
    ///     // Clean up dummy file
    ///     tokio::fs::remove_file(&local_path).await?;
    ///
    ///     session.close().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn download<S, D, L, R, F, Sig>(
        &self,
        src: S,
        dst: D,
        on_length: Option<L>,
        reader_wrapper: Option<F>,
        cancel_signal: Option<Sig>,
    ) -> Result<u64, Error>
    where
        S: AsRef<Path>,
        D: AsRef<Path>,
        R: AsyncRead + Send + Unpin,
        L: FnOnce(u64),
        F: FnOnce(russh_sftp::client::fs::File) -> R,
        Sig: Future<Output = ()> + Unpin,
    {
        let src = src.as_ref();
        let dst = dst.as_ref();
        let src_str = src.to_string_lossy().to_string();

        let sftp = self.prepare_sftp_session().await?;

        // Open remote file for reading
        let remote_file = sftp
            .open_with_flags(&src_str, OpenFlags::READ)
            .await
            .with_context(|_| error::OpenRemoteFileSnafu { path: src_str.clone() })?;

        // Create local file
        let mut local_file =
            LocalFile::create(dst).await.context(error::OpenLocalFileSnafu { path: dst })?;

        if let Some(on_length) = on_length {
            let _unused = remote_file
                .metadata()
                .await
                .inspect(|metadata| {
                    on_length(metadata.len());
                })
                .context(error::OpenRemoteFileSnafu { path: src_str.clone() })?;
        }

        // Wrap writer if provided (similar to reader_wrapper in upload)
        let mut remote_file = match reader_wrapper {
            Some(wrapper) => AsyncEither::Left(wrapper(remote_file)),
            None => AsyncEither::Right(remote_file),
        };

        // Create the copy future
        let copy_task = tokio::io::copy(&mut remote_file, &mut local_file).boxed();

        let n = match cancel_signal {
            Some(sig) => match future::select(copy_task, sig).await {
                future::Either::Left((copy_res, _)) => {
                    copy_res.context(error::TransferDataSnafu { path: dst })?
                }
                future::Either::Right((..)) => return Err(Error::Cancelled),
            },
            None => copy_task.await.context(error::TransferDataSnafu { path: dst })?,
        };

        // Ensure data is flushed to disk
        let _ = local_file.shutdown().await.ok();

        Ok(n)
    }

    /// Closes the SSH session.
    ///
    /// This sends a disconnect message to the remote host and cleans up the
    /// session.
    ///
    /// # Errors
    ///
    /// This function returns an `Error` if:
    /// - Disconnecting the session fails (`error::DisconnectSessionSnafu`).
    ///
    /// # Returns
    ///
    /// A `Result` indicating success or an `Error` on failure.
    ///
    /// # Example
    /// ```no_run
    /// use std::path::Path;
    /// use russh::keys::PrivateKey;
    /// use crate::ssh::{session::Session, error};
    /// use snafu::ResultExt;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let private_key_path = Path::new("id_rsa");
    ///     let private_key = PrivateKey::read_pkcs8(private_key_path, None)
    ///         .await
    ///         .context(error::ReadPrivateKeySnafu)?;
    ///
    ///     let session = Session::connect(private_key, "user", "localhost:22")
    ///         .await?;
    ///
    ///     println!("Session established, now closing...");
    ///     session.close().await?;
    ///     println!("Session closed.");
    ///     Ok(())
    /// }
    /// ```
    pub async fn close(self) -> Result<(), Error> {
        self.session
            .disconnect(Disconnect::ByApplication, "", "English")
            .await
            .context(error::DisconnectSessionSnafu)?;
        Ok(())
    }

    /// Prepares and returns an SFTP session for file transfer operations.
    ///
    /// This internal helper function opens a new channel and requests the SFTP
    /// subsystem.
    ///
    /// # Errors
    ///
    /// This function returns an `Error` if:
    /// - A new channel cannot be opened or the SFTP subsystem request fails
    ///   (`error::OpenSftpSnafu`).
    /// - The SFTP session itself cannot be initialized
    ///   (`error::OpenSftpSessionSnafu`).
    ///
    /// # Returns
    ///
    /// A `Result` containing the `SftpSession` on success, or an `Error` on
    /// failure.
    async fn prepare_sftp_session(&self) -> Result<SftpSession, Error> {
        let channel = self.session.channel_open_session().await.context(error::OpenSftpSnafu)?;
        channel.request_subsystem(true, "sftp").await.context(error::OpenSftpSnafu)?;

        SftpSession::new(channel.into_stream()).await.with_context(|_| error::OpenSftpSessionSnafu)
    }
}
