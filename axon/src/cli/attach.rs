use std::time::Duration;

use clap::Args;
use futures::{SinkExt, channel::mpsc::Sender};
use k8s_openapi::api::core::v1::Pod;
use kube::{
    Api,
    api::{AttachParams, TerminalSize},
};
use snafu::{OptionExt, ResultExt};
use tokio::signal;

use crate::{
    config::Config,
    error::{self, Error},
    ext::{ApiPodExt, PodExt},
    ui::terminal::TerminalRawModeGuard,
};

#[derive(Args, Clone)]
pub struct AttachCommand {
    #[arg(short, long, help = "Namespace of the pod")]
    pub namespace: Option<String>,

    #[arg(short = 'p', long = "pod-name", help = "Name of the pod to attach to")]
    pub pod_name: Option<String>,

    #[arg(short = 's', long = "shell", help = "Interactive shell used to attach container")]
    pub interactive_shell: Vec<String>,

    #[arg(
        short = 't',
        long = "timeout-seconds",
        default_value = "15",
        help = "The maximum time in seconds to wait before timing out"
    )]
    pub timeout_secs: u64,
}

impl AttachCommand {
    pub async fn run(self, kube_client: kube::Client, config: Config) -> Result<(), Error> {
        let Self { namespace, pod_name, interactive_shell, timeout_secs } = self;

        // Resolve Identity
        let namespace = namespace
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| kube_client.default_namespace().to_string());

        let pod_name =
            pod_name.filter(|s| !s.is_empty()).unwrap_or_else(|| config.default_pod_name.clone());

        // Resolve Pod API & Status
        let api = Api::<Pod>::namespaced(kube_client, &namespace);
        let pod = api
            .await_running_status(&pod_name, &namespace, Duration::from_secs(timeout_secs))
            .await?;

        // Resolve Shell
        let shell =
            if interactive_shell.is_empty() { pod.interactive_shell() } else { interactive_shell };

        // Delegate behavior
        PodConsole::new(api, pod_name, namespace, shell).attach().await
    }
}

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

    pub async fn attach(self) -> Result<(), Error> {
        let Self { api, pod_name, namespace, shell } = self;

        // Setup Raw Mode Guard
        let _raw_mode_guard = TerminalRawModeGuard::setup()?;

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

        // Extract Streams
        let pod_stdout =
            attached.stdout().context(error::GetPodStreamSnafu { stream: "stdout" })?;
        let pod_stdin = attached.stdin().context(error::GetPodStreamSnafu { stream: "stdin" })?;
        let term_tx = attached.terminal_size().context(error::GetTerminalSizeWriterSnafu)?;

        // Handle Terminal Resizing
        let cancel_token = tokio_util::sync::CancellationToken::new();
        let mut terminal_size_handle =
            tokio::spawn(handle_terminal_size(term_tx, cancel_token.clone()));

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

        // 6. Cleanup
        cancel_token.cancel();
        let _unused = terminal_size_handle.await;
        let _unused = attached.join().await;

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
