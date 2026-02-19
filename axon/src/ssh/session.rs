use std::{path::Path, sync::Arc, time::Duration};

use russh::{
    ChannelMsg, Disconnect, client,
    keys::{PrivateKey, PublicKey, key::PrivateKeyWithHashAlg},
};
use russh_sftp::{client::SftpSession, protocol::OpenFlags};
use snafu::{IntoError, ResultExt};
use tokio::{
    fs::File as LocalFile,
    io::{AsyncReadExt, AsyncWriteExt},
    net::ToSocketAddrs,
};

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

        let (width, height) = crossterm::terminal::size().context(error::GetTerminalSizeSnafu)?;

        channel
            .request_pty(
                false,
                &std::env::var("TERM").unwrap_or_else(|_| "xterm".into()),
                u32::from(width),
                u32::from(height),
                0,
                0,
                &[],
            )
            .await
            .context(error::RequestPtySnafu)?;

        channel.exec(true, command).await.context(error::ExecuteCommandSnafu)?;

        let code;
        let mut stdin = tokio_fd::AsyncFd::try_from(0).context(error::InitializeStdioSnafu)?;
        let mut stdout = tokio_fd::AsyncFd::try_from(1).context(error::InitializeStdioSnafu)?;
        let mut buf = vec![0; 1024];
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

    pub async fn upload<S, D>(&self, src: S, dst: D) -> Result<u64, Error>
    where
        S: AsRef<Path>,
        D: AsRef<Path>,
    {
        let src = src.as_ref();
        let dst = dst.as_ref();

        let local_file =
            LocalFile::open(src).await.context(error::OpenLocalFileSnafu { path: src })?;

        // Get file size for progress bar
        let metadata =
            local_file.metadata().await.context(error::OpenLocalFileSnafu { path: src })?;

        let dst_str = dst.to_string_lossy().to_string();

        let sftp = self.prepare_sftp_session().await?;
        let mut remote_file = sftp
            .open_with_flags(&dst_str, OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE)
            .await
            .map_err(|source| Error::OpenRemoteFile { path: dst_str, source })?;

        let pb = create_progress_bar(metadata.len(), "Uploading");

        let mut local_file = pb.wrap_async_read(local_file);
        let n = tokio::io::copy(&mut local_file, &mut remote_file)
            .await
            .context(error::TransferDataSnafu { path: src })?;
        let _ = remote_file.shutdown().await.ok();

        pb.finish_with_message("Upload complete");

        Ok(n)
    }

    pub async fn download<S, D>(&self, src: S, dst: D) -> Result<u64, Error>
    where
        S: AsRef<Path>,
        D: AsRef<Path>,
    {
        let src = src.as_ref();
        let dst = dst.as_ref();

        let src_str = src.to_string_lossy().to_string();

        let sftp = self.prepare_sftp_session().await?;
        let remote_file = sftp
            .open_with_flags(&src_str, OpenFlags::READ)
            .await
            .with_context(|_| error::OpenRemoteFileSnafu { path: src_str.clone() })?;

        // Get remote metadata for progress bar
        let remote_meta = remote_file
            .metadata()
            .await
            .with_context(|_| error::OpenRemoteFileSnafu { path: src_str.clone() })?;

        let mut local_file =
            LocalFile::create(dst).await.context(error::OpenLocalFileSnafu { path: dst })?;

        let pb = create_progress_bar(remote_meta.len(), "Downloading");
        let mut remote_file = pb.wrap_async_read(remote_file);
        let n = tokio::io::copy(&mut remote_file, &mut local_file)
            .await
            .context(error::TransferDataSnafu { path: dst })?;
        pb.finish_with_message("Download complete");

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

fn create_progress_bar(len: u64, msg: &'static str) -> indicatif::ProgressBar {
    let pb = indicatif::ProgressBar::new(len);
    pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] \
                 {bytes}/{total_bytes} ({eta}) {msg}",
            )
            .unwrap()
            .progress_chars("#>-"),
    );
    pb.set_message(msg);
    pb
}
