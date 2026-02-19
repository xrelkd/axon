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

#[derive(Default)]
struct Client {}

impl client::Handler for Client {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

pub struct Session {
    session: client::Handle<Client>,
}

impl Session {
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
        let mut stdin = tokio_fd::AsyncFd::try_from(0).context(error::InitializeStdioSnafu)?;
        let mut stdout = tokio_fd::AsyncFd::try_from(1).context(error::InitializeStdioSnafu)?;
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

    pub async fn close(self) -> Result<(), Error> {
        self.session
            .disconnect(Disconnect::ByApplication, "", "English")
            .await
            .context(error::DisconnectSessionSnafu)?;
        Ok(())
    }

    async fn prepare_sftp_session(&self) -> Result<SftpSession, Error> {
        let channel = self.session.channel_open_session().await.context(error::OpenSftpSnafu)?;
        channel.request_subsystem(true, "sftp").await.context(error::OpenSftpSnafu)?;

        SftpSession::new(channel.into_stream()).await.with_context(|_| error::OpenSftpSessionSnafu)
    }
}
