use std::{sync::Arc, time::Duration};

use russh::{
    ChannelMsg, Disconnect, client,
    keys::{PrivateKey, PublicKey, key::PrivateKeyWithHashAlg},
};
use snafu::{IntoError, ResultExt};
use tokio::{
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

    pub async fn close(self) -> Result<(), Error> {
        self.session
            .disconnect(Disconnect::ByApplication, "", "English")
            .await
            .context(error::DisconnectSessionSnafu)?;
        Ok(())
    }
}
