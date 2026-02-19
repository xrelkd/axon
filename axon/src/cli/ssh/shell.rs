use std::{path::PathBuf, time::Duration};

use clap::{ArgAction, Args};
use k8s_openapi::api::core::v1::Pod;
use kube::Api;
use sigfinn::{ExitStatus, LifecycleManager};

use crate::{
    config::Config,
    error::{self, Error},
    ext::{ApiPodExt, PodExt},
    port_forwarder::PortForwarderBuilder,
    ssh,
    ui::terminal::TerminalRawModeGuard,
};

const DEFAULT_SSH_PORT: u16 = 22;

#[derive(Args, Clone)]
pub struct ShellCommand {
    #[arg(short, long, help = "Namespace of the pod")]
    namespace: Option<String>,

    #[arg(short = 'p', long = "pod-name", help = "Name of the pod to attach to")]
    pod_name: Option<String>,

    #[arg(
        short = 't',
        long = "timeout-seconds",
        default_value = "15",
        help = "The maximum time in seconds to wait before timing out"
    )]
    timeout_secs: u64,

    #[arg(short = 'i', long = "ssh-private-key-file", help = "File path of a SSH private key")]
    ssh_private_key_file: Option<PathBuf>,

    #[arg(short = 'u', long = "user", default_value = "root", help = "User name")]
    user: String,

    #[arg(action = ArgAction::Append, default_value = "/bin/zsh", help = "Command")]
    command: Vec<String>,
}

impl ShellCommand {
    pub async fn run(self, kube_client: kube::Client, config: Config) -> Result<(), Error> {
        let Self { namespace, pod_name, timeout_secs, ssh_private_key_file, user, command } = self;

        let ssh_private_key = {
            let ((Some(ssh_private_key_file), _) | (None, Some(ssh_private_key_file))) =
                (ssh_private_key_file, config.ssh_private_key_file_path)
            else {
                return error::NoSshPrivateKeyProvidedSnafu.fail();
            };
            ssh::load_secret_key(ssh_private_key_file, None).await?
        };

        let namespace = namespace
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| kube_client.default_namespace().to_string());
        let pod_name =
            pod_name.filter(|s| !s.is_empty()).unwrap_or_else(|| config.default_pod_name.clone());

        let api = Api::<Pod>::namespaced(kube_client, &namespace);
        let pod = api
            .await_running_status(&pod_name, &namespace, Duration::from_secs(timeout_secs))
            .await?;
        let remote_port = pod.service_ports().ssh.unwrap_or(DEFAULT_SSH_PORT);
        let remote_command = if command.is_empty() { pod.interactive_shell() } else { command };

        let lifecycle_manager = LifecycleManager::<Error>::new();

        let (handle, ssh_local_socket_addr_receiver) = {
            let (sender, receiver) = tokio::sync::oneshot::channel();
            let on_ready = move |socket_addr| {
                let _unused = sender.send(socket_addr);
            };
            let handle =
                lifecycle_manager.spawn("port-forwarder", move |shutdown_signal| async move {
                    let result = PortForwarderBuilder::new(api, pod_name, remote_port)
                        .on_ready(on_ready)
                        .build()
                        .run(shutdown_signal)
                        .await;
                    match result {
                        Ok(()) => ExitStatus::Success,
                        Err(err) => ExitStatus::Error(Error::from(err)),
                    }
                });
            (handle, receiver)
        };

        let _handle = lifecycle_manager.spawn("ssh-client", move |_| async move {
            let result = SshClientRunner {
                handle,
                addr_receiver: ssh_local_socket_addr_receiver,
                ssh_private_key,
                user,
                remote_command,
            }
            .run()
            .await;
            match result {
                Ok(()) => ExitStatus::Success,
                Err(err) => ExitStatus::Error(err),
            }
        });

        if let Ok(Err(err)) = lifecycle_manager.serve().await {
            tracing::error!("{err}");
            Err(err)
        } else {
            Ok(())
        }
    }
}

struct SshClientRunner {
    handle: sigfinn::Handle<Error>,
    addr_receiver: tokio::sync::oneshot::Receiver<std::net::SocketAddr>,
    ssh_private_key: russh::keys::PrivateKey,
    user: String,
    remote_command: Vec<String>,
}

impl SshClientRunner {
    async fn run(self) -> Result<(), Error> {
        let Self { handle, addr_receiver, ssh_private_key, user, remote_command } = self;

        // Automatically shuts down the port forwarder when this scope ends
        let _handle_guard = HandleGuard::from(handle);

        let ssh_local_socket_addr = addr_receiver.await.map_err(|_| {
            error::GenericSnafu { message: "SSH local socket address receiver failed" }.build()
        })?;

        let session = ssh::Session::connect(ssh_private_key, user, ssh_local_socket_addr).await?;

        // Enter raw mode to handle TTY interactions correctly
        let _raw_mode_guard = TerminalRawModeGuard::setup()?;

        let escaped_command = remote_command
            .into_iter()
            .map(|x| shell_escape::escape(x.into()))
            .collect::<Vec<_>>()
            .join(" ");

        let call_result = session.call(&escaped_command).await;

        // Attempt to close the session cleanly
        let close_result = session.close().await;

        // Return the execution error if it exists, otherwise the closing error
        call_result.map(|_| ()).map_err(Error::from)?;
        close_result.map_err(Error::from)
    }
}

struct HandleGuard {
    handle: sigfinn::Handle<Error>,
}

impl From<sigfinn::Handle<Error>> for HandleGuard {
    fn from(handle: sigfinn::Handle<Error>) -> Self { Self { handle } }
}

impl Drop for HandleGuard {
    fn drop(&mut self) { self.handle.shutdown(); }
}
