use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    time::Duration,
};

use clap::{ArgAction, Args};
use k8s_openapi::api::core::v1::Pod;
use kube::Api;
use sigfinn::LifecycleManager;

use crate::{
    config::Config,
    error::{self, Error},
    ext::{ApiPodExt, PodExt},
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

        let (ssh_local_socket_addr_sender, ssh_local_socket_addr_receiver) =
            tokio::sync::oneshot::channel();
        let handle = lifecycle_manager.handle();
        let handle = lifecycle_manager.spawn("port-forwarder", move |shutdown_signal| async move {
            let local_socket_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
            let on_ready = move |socket_addr| {
                let _unused = ssh_local_socket_addr_sender.send(socket_addr);
            };
            api.port_forward(
                &pod_name,
                local_socket_addr,
                remote_port,
                handle,
                shutdown_signal,
                on_ready,
            )
            .await
        });
        let _handle = lifecycle_manager.spawn("ssh-client", move |_shutdown_signal| async move {
            let _handle_guard = HandleGuard::from(handle);
            let ssh_local_socket_addr = match ssh_local_socket_addr_receiver.await {
                Ok(addr) => addr,
                Err(_err) => return sigfinn::ExitStatus::Success,
            };

            let session =
                match ssh::Session::connect(ssh_private_key, user, ssh_local_socket_addr).await {
                    Ok(session) => session,
                    Err(err) => return sigfinn::ExitStatus::Error(Error::from(err)),
                };
            let _raw_mode_guard = match TerminalRawModeGuard::setup() {
                Ok(guard) => guard,
                Err(err) => return sigfinn::ExitStatus::Error(Error::from(err)),
            };

            // arguments are escaped manually since the SSH protocol doesn't support quoting
            let remote_command = remote_command
                .into_iter()
                .map(|x| shell_escape::escape(x.into()))
                .collect::<Vec<_>>()
                .join(" ");
            let _exit_code = match session.call(&remote_command).await {
                Ok(exit_code) => exit_code,
                Err(err) => {
                    let _unused = session.close().await;
                    return sigfinn::ExitStatus::Error(Error::from(err));
                }
            };
            match session.close().await {
                Ok(()) => sigfinn::ExitStatus::Success,
                Err(err) => sigfinn::ExitStatus::Error(Error::from(err)),
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

struct HandleGuard {
    handle: sigfinn::Handle<Error>,
}

impl From<sigfinn::Handle<Error>> for HandleGuard {
    fn from(handle: sigfinn::Handle<Error>) -> Self { Self { handle } }
}

impl Drop for HandleGuard {
    fn drop(&mut self) { self.handle.shutdown(); }
}
