//! Kubernetes Pod Interactive Console.
//!
//! This module provides the ability to attach to a running Pod's container and
//! interact with it via a terminal-like interface. It handles raw mode terminal
//! settings, standard I/O streaming, and dynamic terminal window resizing
//! (SIGWINCH).

mod error;

use futures::{FutureExt, SinkExt, channel::mpsc::Sender};
use k8s_openapi::api::core::v1::Pod;
use kube::{
    Api,
    api::{AttachParams, TerminalSize},
};
use snafu::{OptionExt, ResultExt};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    signal,
};

pub use self::error::Error;
use crate::ui::terminal::TerminalRawModeGuard;

/// A controller for managing an interactive terminal session with a Kubernetes
/// Pod.
///
/// `PodConsole` encapsulates the logic required to establish a TTY connection
/// to a specific Pod and synchronize local terminal input/output with the
/// remote container.
#[derive(Clone, Debug)]
pub struct PodConsole {
    /// The Kubernetes API client for Pods.
    api: Api<Pod>,
    /// The name of the target Pod.
    pod_name: String,
    /// The namespace where the Pod is located.
    namespace: String,
    /// The command to run within the container (e.g., `["/bin/sh"]`).
    shell: Vec<String>,
}

impl PodConsole {
    /// Creates a new `PodConsole` instance.
    ///
    /// # Arguments
    ///
    /// * `api` - The Kubernetes API client for Pods.
    /// * `pod_name` - The name of the target Pod.
    /// * `namespace` - The namespace where the Pod is located.
    /// * `shell` - An iterator of strings representing the command to run
    ///   (e.g., `["/bin/sh"]`).
    ///
    /// # Returns
    ///
    /// A new `PodConsole` instance.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use kube::{Client, Api};
    /// use k8s_openapi::api::core::v1::Pod;
    /// use axon::pod_console::PodConsole;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = Client::try_default().await?;
    ///     let api: Api<Pod> = Api::namespaced(client, "default");
    ///     let console = PodConsole::new(api, "my-pod", "default", vec!["/bin/bash"]);
    ///     Ok(())
    /// }
    /// ```
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

    /// Establishes and manages an interactive terminal session with the
    /// Kubernetes Pod.
    ///
    /// This method sets the local terminal to raw mode, connects to the Pod,
    /// and pipes I/O between the local terminal and the remote container.
    /// It also spawns a background task to handle terminal window resizing
    /// (`SIGWINCH`). The session continues until the Pod connection is
    /// closed, an I/O error occurs, or the terminal size handling task
    /// finishes unexpectedly.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] if:
    ///
    /// * The local terminal fails to enter raw mode
    ///   (`TerminalRawModeGuard::setup`).
    /// * The connection to the Kubernetes API fails during the `exec` call
    ///   (`error::AttachPodSnafu`).
    /// * The terminal size writer cannot be obtained
    ///   (`error::GetTerminalSizeWriterSnafu`).
    /// * Standard I/O streams from the Pod cannot be retrieved
    ///   (`error::GetPodStreamSnafu`).
    /// * Local standard I/O handles cannot be initialized
    ///   (`error::InitializeStdioSnafu`).
    /// * An I/O error occurs during data transfer between local and remote
    ///   streams (`error::CopyIoSnafu`).
    ///
    /// # Example
    ///
    /// ```no_run
    /// use kube::{Client, Api};
    /// use k8s_openapi::api::core::v1::Pod;
    /// use axon::pod_console::PodConsole;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let client = Client::try_default().await?;
    ///     let api: Api<Pod> = Api::namespaced(client, "default");
    ///     let console = PodConsole::new(api, "my-pod", "default", vec!["/bin/bash"]);
    ///
    ///     println!("Connecting to pod 'my-pod' in namespace 'default'...");
    ///     console.run().await?;
    ///     println!("Disconnected from pod.");
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn run(self) -> Result<(), Error> {
        let _raw_mode_guard = TerminalRawModeGuard::setup()?;
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
        let term_tx = attached.terminal_size().context(error::GetTerminalSizeWriterSnafu)?;
        let mut terminal_size_handle =
            tokio::spawn(handle_terminal_size(term_tx, cancel_token.clone()));

        let mut pod_stdout =
            attached.stdout().context(error::GetPodStreamSnafu { stream: "stdout" })?;
        let mut pod_stdin =
            attached.stdin().context(error::GetPodStreamSnafu { stream: "stdin" })?;

        let mut local_stdin = tokio_fd::AsyncFd::try_from(0)
            .context(error::InitializeStdioSnafu { stream: "stdin" })?;
        let mut local_stdout = tokio_fd::AsyncFd::try_from(1)
            .context(error::InitializeStdioSnafu { stream: "stdout" })?;

        let mut in_buffer = vec![0u8; 4096];
        let mut out_buffer = vec![0u8; 4096];

        let mut attached_join = attached.join().fuse().boxed();

        loop {
            tokio::select! {
                _ = &mut attached_join => {
                    tracing::debug!("Pod connection closed by remote");
                    break;
                },
                res = local_stdin.read(&mut in_buffer) => {
                    match res {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            pod_stdin.write_all(&in_buffer[..n]).await.context(error::CopyIoSnafu)?;
                            pod_stdin.flush().await.context(error::CopyIoSnafu)?;
                        }
                    }
                },
                res = pod_stdout.read(&mut out_buffer) => {
                    match res {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            local_stdout.write_all(&out_buffer[..n]).await.context(error::CopyIoSnafu)?;
                            local_stdout.flush().await.context(error::CopyIoSnafu)?;
                        }
                    }
                },
                res = &mut terminal_size_handle => {
                    tracing::debug!("Terminal size task finished: {:?}", res);
                    break;
                }
            }
        }

        cancel_token.cancel();
        let _unused = terminal_size_handle.await;

        Ok(())
    }
}

/// Monitors for terminal resize events and notifies the Kubernetes API.
///
/// This function listens for the `SIGWINCH` signal on Unix systems. When the
/// terminal is resized, it fetches the new dimensions and sends them through
/// the provided channel to update the remote container's TTY size.
///
/// # Arguments
///
/// * `channel` - A `Sender` to send `TerminalSize` updates to the Kubernetes
///   API.
/// * `cancel_token` - A `CancellationToken` to signal the task to gracefully
///   shut down.
///
/// # Returns
///
/// A `Result` indicating success or an `Error` if an issue occurred.
///
/// # Errors
///
/// Returns an [`Error`] if:
///
/// * The initial terminal size cannot be retrieved
///   (`error::GetTerminalSizeSnafu`).
/// * Sending the initial terminal size over the channel fails
///   (`Error::ChangeTerminalSize`).
/// * The `SIGWINCH` signal stream cannot be created
///   (`error::CreateSignalStreamSnafu`).
/// * Retrieving terminal size after a resize event fails
///   (`error::GetTerminalSizeSnafu`).
/// * Sending a subsequent terminal size update over the channel fails
///   (`Error::ChangeTerminalSize`).
///
/// # Example
///
/// ```no_run
/// use tokio::sync::mpsc;
/// use kube::api::TerminalSize;
/// use tokio_util::sync::CancellationToken;
/// use axon_pod_console::Error; // Assuming `axon_pod_console` is your crate name
///
/// async fn my_terminal_resizer_task(
///     mut sender: mpsc::Sender<TerminalSize>,
///     cancel: CancellationToken,
/// ) -> Result<(), Error> {
///     // In a real application, 'sender' would be connected to the Kube client.
///     // For this example, we just show how to call handle_terminal_size.
///     axon_pod_console::handle_terminal_size(sender, cancel).await
/// }
///
/// #[tokio::main]
/// async fn main() {
///     let (tx, mut rx) = mpsc::channel(1);
///     let cancel_token = CancellationToken::new();
///
///     let _resize_handle = tokio::spawn(my_terminal_resizer_task(tx, cancel_token.clone()));
///
///     // In a real scenario, you'd have other logic here that eventually
///     // calls cancel_token.cancel() to stop the resize task.
///     tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
///     cancel_token.cancel();
///
///     println!("Terminal resize task simulated.");
/// }
/// ```
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
