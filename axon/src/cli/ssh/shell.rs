//! This module defines the `ShellCommand` struct and its associated logic for
//! establishing an interactive SSH shell session to a Kubernetes pod.
//!
//! It handles parsing command-line arguments, resolving target resources,
//! setting up SSH keys, performing port forwarding, and executing the SSH
//! client.

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

/// Represents the command-line arguments for the `shell` subcommand.
///
/// This struct parses arguments related to connecting to a Kubernetes pod via
/// SSH, including namespace, pod name, timeouts, SSH key paths, user, and the
/// command to execute within the shell.
#[derive(Args, Clone)]
pub struct ShellCommand {
    /// Kubernetes namespace of the target pod.
    /// If not specified, the default namespace will be used.
    #[arg(
        short,
        long,
        help = "Kubernetes namespace of the target pod. If not specified, the default namespace \
                will be used."
    )]
    pub namespace: Option<String>,

    /// Name of the temporary pod to open an SSH shell into.
    /// If not specified, Axon's default pod name will be used.
    #[arg(
        short = 'p',
        long = "pod-name",
        help = "Name of the temporary pod to open an SSH shell into. If not specified, Axon's \
                default pod name will be used."
    )]
    pub pod_name: Option<String>,

    /// The maximum time in seconds to wait for the pod to be running before
    /// timing out.
    #[arg(
        short = 't',
        long = "timeout-seconds",
        default_value = "15",
        help = "The maximum time in seconds to wait for the pod to be running before timing out."
    )]
    pub timeout_secs: u64,

    /// Path to the SSH private key file for authentication.
    /// If not specified, Axon will look for `sshPrivateKeyFilePath` in the
    /// configuration.
    #[arg(
        short = 'i',
        long = "ssh-private-key-file",
        help = "Path to the SSH private key file for authentication. If not specified, Axon will \
                look for `sshPrivateKeyFilePath` in the configuration."
    )]
    pub ssh_private_key_file: Option<PathBuf>,

    /// User name to connect as via SSH on the remote pod.
    #[arg(
        short = 'u',
        long = "user",
        default_value = "root",
        help = "User name to connect as via SSH on the remote pod."
    )]
    pub user: String,

    /// The command and its arguments to execute as the interactive SSH shell.
    /// If not specified, Axon will attempt to detect the shell.
    #[arg(
        action = ArgAction::Append,
        default_value = "/bin/zsh",
        help = "The command and its arguments to execute as the interactive SSH shell. \
                If not specified, Axon will attempt to detect the shell."
    )]
    pub command: Vec<String>,
}

impl ShellCommand {
    /// Executes the SSH shell command, connecting to a specified Kubernetes
    /// pod.
    ///
    /// This asynchronous function performs the following steps:
    /// 1. Resolves the target Kubernetes namespace and pod name.
    /// 2. Loads the SSH key pair from the specified path or configuration.
    /// 3. Waits for the target pod to reach a running state within the given
    ///    timeout.
    /// 4. Determines the remote SSH port and the command to execute on the pod.
    /// 5. Uploads the SSH public key to the pod for authentication.
    /// 6. Sets up port forwarding to the pod's SSH service.
    /// 7. Spawns an SSH client runner task to establish and manage the SSH
    ///    session.
    /// 8. Manages the lifecycle of the port forwarding and SSH client, handling
    ///    potential errors.
    ///
    /// # Arguments
    ///
    /// * `self` - The `ShellCommand` instance containing parsed command-line
    ///   arguments.
    /// * `kube_client` - A Kubernetes client used to interact with the API
    ///   server.
    /// * `config` - The application's configuration, including default SSH key
    ///   paths.
    ///
    /// # Errors
    ///
    /// This function can return an `Error` in the following cases:
    /// * If the SSH key pair cannot be loaded.
    /// * If the target pod cannot be found or does not reach a running state
    ///   within the timeout.
    /// * If the SSH public key cannot be uploaded to the pod.
    /// * If port forwarding setup fails.
    /// * If the SSH client fails to connect or execute the command.
    /// * If the SSH local socket address receiver fails to provide an address.
    ///
    /// # Panics
    ///
    /// This function may panic if `kube::Api::namespaced` is called with an
    /// invalid namespace, though this should be prevented by prior
    /// validation. It also calls `unwrap()` on the result of
    /// `lifecycle_manager.serve()`, which would panic if the `serve` method
    /// returns `Ok(Err(err))` and `lifecycle_manager.serve()` itself returns
    /// `Err`.
    pub async fn run(self, kube_client: kube::Client, config: Config) -> Result<(), Error> {
        let Self { namespace, pod_name, timeout_secs, ssh_private_key_file, user, command } = self;

        // Resolve Identity
        let ResolvedResources { namespace, pod_name } =
            ResourceResolver::from((&kube_client, &config)).resolve(namespace, pod_name);

        let (ssh_private_key, ssh_public_key) = ssh::resolve_ssh_key_pair(
            [ssh_private_key_file.as_ref(), config.ssh_private_key_file_path.as_ref()]
                .iter()
                .flatten(),
        )
        .await?;

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

/// A runner responsible for establishing and managing an SSH client session.
///
/// This struct holds the necessary information to connect to a remote SSH
/// server (via a local forwarded port) and execute a command.
struct SshClientRunner {
    /// A `sigfinn::Handle` to manage the lifecycle of related tasks,
    /// specifically for graceful shutdown of port forwarding.
    handle: sigfinn::Handle<Error>,
    /// The local socket address to connect to for the SSH session,
    /// typically established via port forwarding.
    socket_addr: SocketAddr,
    /// The SSH private key used for authentication with the remote host.
    ssh_private_key: russh::keys::PrivateKey,
    /// The username to use for the SSH connection.
    user: String,
    /// The command and its arguments to execute on the remote host.
    command: Vec<String>,
}

impl SshClientRunner {
    /// Runs the SSH client, connecting to the remote host and executing the
    /// specified command.
    ///
    /// This function performs the following actions:
    /// 1. Creates a `HandleGuard` to ensure the associated port forwarder is
    ///    shut down when this runner's scope ends.
    /// 2. Establishes an SSH session to the `socket_addr` using the provided
    ///    private key and user.
    /// 3. Enters terminal raw mode to correctly handle interactive SSH shell
    ///    input/output.
    /// 4. Escapes the command arguments and joins them into a single string for
    ///    execution.
    /// 5. Executes the command on the remote SSH session.
    /// 6. Attempts to gracefully close the SSH session.
    /// 7. Returns any error encountered during command execution or session
    ///    closing.
    ///
    /// # Arguments
    ///
    /// * `self` - The `SshClientRunner` instance containing connection details
    ///   and the command.
    ///
    /// # Errors
    ///
    /// This function can return an `Error` in the following situations:
    /// * If establishing the SSH session fails (e.g., connection refused,
    ///   authentication issues).
    /// * If setting up terminal raw mode fails.
    /// * If executing the remote command fails.
    /// * If closing the SSH session fails.
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
