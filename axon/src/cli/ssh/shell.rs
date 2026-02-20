use std::{net::SocketAddr, path::PathBuf, time::Duration};

use clap::{ArgAction, Args};
use k8s_openapi::api::core::v1::Pod;
use kube::Api;
use sigfinn::{ExitStatus, LifecycleManager};

use crate::{
    cli::{
        Error, error,
        internal::{ApiPodExt, ResolvedResources, ResourceResolver},
        ssh::internal::{Configurator, DEFAULT_SSH_PORT, HandleGuard, setup_port_forwarding},
    },
    config::Config,
    ext::PodExt,
    ssh,
    ui::terminal::TerminalRawModeGuard,
};

#[derive(Args, Clone)]
pub struct ShellCommand {
    #[arg(
        short,
        long,
        help = "Kubernetes namespace of the target pod. If not specified, the default namespace \
                will be used."
    )]
    namespace: Option<String>,

    #[arg(
        short = 'p',
        long = "pod-name",
        help = "Name of the temporary pod to open an SSH shell into. If not specified, Axon's \
                default pod name will be used."
    )]
    pod_name: Option<String>,

    #[arg(
        short = 't',
        long = "timeout-seconds",
        default_value = "15",
        help = "The maximum time in seconds to wait for the pod to be running before timing out."
    )]
    timeout_secs: u64,

    #[arg(
        short = 'i',
        long = "ssh-private-key-file",
        help = "Path to the SSH private key file for authentication. If not specified, Axon will \
                look for `sshPrivateKeyFilePath` in the configuration."
    )]
    ssh_private_key_file: Option<PathBuf>,

    #[arg(
        short = 'u',
        long = "user",
        default_value = "root",
        help = "User name to connect as via SSH on the remote pod."
    )]
    user: String,

    #[arg(
        action = ArgAction::Append,
        default_value = "/bin/zsh",
        help = "The command and its arguments to execute as the interactive SSH shell. \
                If not specified, Axon will attempt to detect the shell."
    )]
    command: Vec<String>,
}

impl ShellCommand {
    pub async fn run(self, kube_client: kube::Client, config: Config) -> Result<(), Error> {
        let Self { namespace, pod_name, timeout_secs, ssh_private_key_file, user, command } = self;

        // Resolve Identity
        let ResolvedResources { namespace, pod_name } =
            ResourceResolver::from((&kube_client, &config)).resolve(namespace, pod_name);

        let (ssh_private_key, ssh_public_key) =
            ssh::load_ssh_key_pair(ssh_private_key_file, config.ssh_private_key_file_path).await?;

        let api = Api::<Pod>::namespaced(kube_client, &namespace);
        let pod = api
            .await_running_status(&pod_name, &namespace, Duration::from_secs(timeout_secs))
            .await?;
        let remote_port = pod.service_ports().ssh.unwrap_or(DEFAULT_SSH_PORT);
        let remote_command = if command.is_empty() { pod.interactive_shell() } else { command };

        Configurator::new(api.clone(), &namespace, &pod_name)
            .upload_ssh_key(ssh_public_key)
            .await?;

        let lifecycle_manager = LifecycleManager::<Error>::new();
        let handle = lifecycle_manager.handle();
        let ssh_local_socket_addr_receiver =
            setup_port_forwarding(api, pod_name, remote_port, &handle);
        let _handle = lifecycle_manager.spawn("ssh-client", move |_| async move {
            let socket_addr = match ssh_local_socket_addr_receiver.await {
                Ok(a) => a,
                Err(_err) => {
                    let err =
                        error::GenericSnafu { message: "SSH local socket address receiver failed" }
                            .build();
                    return ExitStatus::Error(err);
                }
            };

            let result = SshClientRunner {
                handle,
                socket_addr,
                ssh_private_key,
                user,
                command: remote_command,
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
    socket_addr: SocketAddr,
    ssh_private_key: russh::keys::PrivateKey,
    user: String,
    command: Vec<String>,
}

impl SshClientRunner {
    async fn run(self) -> Result<(), Error> {
        let Self { handle, socket_addr, ssh_private_key, user, command } = self;

        // Automatically shuts down the port forwarder when this scope ends
        let _handle_guard = HandleGuard::from(handle);

        let session = ssh::Session::connect(ssh_private_key, user, socket_addr).await?;

        // Enter raw mode to handle TTY interactions correctly
        let _raw_mode_guard = TerminalRawModeGuard::setup()?;

        let escaped_command = command
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
