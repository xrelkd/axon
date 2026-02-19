mod error;

use futures::{SinkExt, channel::mpsc::Sender};
use k8s_openapi::api::core::v1::Pod;
use kube::{
    Api,
    api::{AttachParams, TerminalSize},
};
use snafu::{OptionExt, ResultExt};
use tokio::{io::AsyncWriteExt, signal};

pub use self::error::Error;
use crate::ui::terminal::TerminalRawModeGuard;

#[derive(Clone, Debug)]
pub struct PodConsole {
    api: Api<Pod>,
    pod_name: String,
    namespace: String,
    shell: Vec<String>,
}

impl PodConsole {
    pub fn new<I, S>(
        api: Api<Pod>,
        pod_name: impl Into<String>,
        namespace: impl Into<String>,
        shell: I,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            api,
            pod_name: pod_name.into(),
            namespace: namespace.into(),
            shell: shell.into_iter().map(Into::into).collect(),
        }
    }

    pub async fn run(self) -> Result<(), Error> {
        let Self { api, pod_name, namespace, shell } = self;

        // Initiate Exec
        let mut attached = api
            .exec(
                &pod_name,
                shell,
                &AttachParams {
                    stdin: true,
                    stdout: true,
                    stderr: false,
                    tty: true,
                    ..AttachParams::default()
                },
            )
            .await
            .with_context(|_| error::AttachPodSnafu {
                namespace: namespace.clone(),
                pod_name: pod_name.clone(),
            })?;

        // Handle Terminal Resizing
        let cancel_token = tokio_util::sync::CancellationToken::new();
        let mut terminal_size_handle = {
            let term_tx = attached.terminal_size().context(error::GetTerminalSizeWriterSnafu)?;
            tokio::spawn(handle_terminal_size(term_tx, cancel_token.clone()))
        };

        {
            // Extract Streams
            let pod_stdout =
                attached.stdout().context(error::GetPodStreamSnafu { stream: "stdout" })?;
            let pod_stdin =
                attached.stdin().context(error::GetPodStreamSnafu { stream: "stdin" })?;

            let _raw_mode_guard = TerminalRawModeGuard::setup()?;

            // Bidirectional Copy
            let mut pod_combined = tokio::io::join(pod_stdout, pod_stdin);
            let mut local_combined = tokio::io::join(tokio::io::stdin(), tokio::io::stdout());

            tokio::select! {
                result = tokio::io::copy_bidirectional(&mut local_combined, &mut pod_combined) => {
                    if let Err(err) = result && err.kind() != std::io::ErrorKind::BrokenPipe {
                        return Err(err).context(error::CopyBidirectionalIoSnafu);
                    }
                },
                result = &mut terminal_size_handle => {
                    match result {
                        Ok(_) => tracing::info!("End of terminal size stream"),
                        Err(err) => tracing::warn!("Error getting terminal size: {err}")
                    }
                },
            }
            let _unused = local_combined.shutdown().await;
            let _unused = pod_combined.shutdown().await;
        }

        let _unused = attached.join().await;

        // Cleanup
        cancel_token.cancel();
        drop(cancel_token);

        let _unused = terminal_size_handle.await;

        Ok(())
    }
}

// Send the new terminal size to channel when it change
async fn handle_terminal_size(
    mut channel: Sender<TerminalSize>,
    cancel_token: tokio_util::sync::CancellationToken,
) -> Result<(), Error> {
    let (width, height) = crossterm::terminal::size().context(error::GetTerminalSizeSnafu)?;
    channel.send(TerminalSize { height, width }).await.map_err(|_| Error::ChangeTerminalSize)?;

    // create a stream to catch SIGWINCH signal
    let mut signal = signal::unix::signal(signal::unix::SignalKind::window_change())
        .context(error::CreateSignalStreamSnafu)?;

    loop {
        let maybe_signal = tokio::select! {
            () = cancel_token.cancelled() => break,
            maybe_signal = signal.recv() => maybe_signal,
        };

        if maybe_signal.is_some() {
            let (width, height) =
                crossterm::terminal::size().context(error::GetTerminalSizeSnafu)?;
            channel
                .send(TerminalSize { height, width })
                .await
                .map_err(|_| Error::ChangeTerminalSize)?;
        } else {
            break;
        }
    }

    Ok(())
}
