//! This module defines the `GetCommand` structure and its associated logic
//! for downloading files from a remote Kubernetes pod via SSH.

use std::{path::PathBuf, time::Duration};

use clap::Args;
use k8s_openapi::api::core::v1::Pod;
use kube::Api;
use sigfinn::{ExitStatus, LifecycleManager};

use crate::{
    cli::{
        Error, error,
        internal::{ApiPodExt, ResolvedResources, ResourceResolver},
        ssh::internal::{
            Configurator, DEFAULT_SSH_PORT, FileTransfer, FileTransferRunner, setup_port_forwarding,
        },
    },
    config::Config,
    ext::PodExt,
    ssh,
};

/// Represents the command to download a file from a remote pod.
///
/// This struct defines the command-line arguments required to specify
/// the target pod, authentication details, source file path on the pod,
/// and the destination path on the local machine.
#[derive(Args, Clone)]
pub struct GetCommand {
    /// Kubernetes namespace of the target pod. If not specified, the default
    /// namespace will be used.
    #[arg(
        short,
        long,
        help = "Kubernetes namespace of the target pod. If not specified, the default namespace \
                will be used."
    )]
    namespace: Option<String>,

    /// Name of the temporary pod to get the file from. If not specified, Axon's
    /// default pod name will be used.
    #[arg(
        short = 'p',
        long = "pod-name",
        help = "Name of the temporary pod to get the file from. If not specified, Axon's default \
                pod name will be used."
    )]
    pod_name: Option<String>,

    /// The maximum time in seconds to wait for the pod to be running before
    /// timing out.
    #[arg(
        short = 't',
        long = "timeout-seconds",
        default_value = "15",
        help = "The maximum time in seconds to wait for the pod to be running before timing out."
    )]
    timeout_secs: u64,

    /// Path to the SSH private key file for authentication. If not specified,
    /// Axon will look for `sshPrivateKeyFilePath` in the configuration.
    #[arg(
        short = 'i',
        long = "ssh-private-key-file",
        help = "Path to the SSH private key file for authentication. If not specified, Axon will \
                look for `sshPrivateKeyFilePath` in the configuration."
    )]
    ssh_private_key_file: Option<PathBuf>,

    /// User name to connect as via SSH on the remote pod.
    #[arg(
        short = 'u',
        long = "user",
        default_value = "root",
        help = "User name to connect as via SSH on the remote pod."
    )]
    user: String,

    /// Path to the file on the remote pod to download.
    #[arg(help = "Path to the file on the remote pod to download.")]
    source: PathBuf,

    /// Local path where the downloaded file will be saved.
    #[arg(help = "Local path where the downloaded file will be saved.")]
    destination: PathBuf,
}

impl GetCommand {
    /// Executes the file download operation from a Kubernetes pod to the local
    /// filesystem.
    ///
    /// This asynchronous function resolves the target pod, sets up SSH
    /// authentication, establishes port-forwarding, and then initiates the
    /// file transfer.
    ///
    /// # Arguments
    ///
    /// * `self` - The `GetCommand` instance containing all command-line
    ///   arguments.
    /// * `kube_client` - A Kubernetes client used to interact with the API
    ///   server.
    /// * `config` - The application's configuration, potentially containing
    ///   default values.
    ///
    /// # Errors
    ///
    /// This function returns an `Err` if:
    /// * The SSH key pair cannot be loaded.
    /// * The target pod cannot be found or does not reach a running state
    ///   within the specified timeout.
    /// * The SSH configurator fails to upload the public key to the pod.
    /// * Port forwarding setup fails.
    /// * The file transfer operation encounters an error.
    /// * Any underlying Kubernetes API operation fails.
    ///
    /// # Panics
    ///
    /// This function will panic if `pod.service_ports().ssh` returns `None` and
    /// `DEFAULT_SSH_PORT` is not a valid port, or if
    /// `ssh_local_socket_addr_receiver` fails to retrieve the
    /// socket address.
    pub async fn run(self, kube_client: kube::Client, config: Config) -> Result<(), Error> {
        let Self {
            namespace,
            pod_name,
            timeout_secs,
            ssh_private_key_file,
            user,
            source,
            destination,
        } = self;

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

        Configurator::new(api.clone(), &namespace, &pod_name)
            .upload_ssh_key(ssh_public_key)
            .await?;

        let lifecycle_manager = LifecycleManager::<Error>::new();
        let handle = lifecycle_manager.handle();
        let ssh_local_socket_addr_receiver =
            setup_port_forwarding(api, pod_name, remote_port, &handle);
        let _handle = lifecycle_manager.spawn("ssh-client", move |shutdown_signal| async move {
            let socket_addr = match ssh_local_socket_addr_receiver.await {
                Ok(a) => a,
                Err(_err) => {
                    let err =
                        error::GenericSnafu { message: "SSH local socket address receiver failed" }
                            .build();
                    return ExitStatus::Error(err);
                }
            };

            let result = FileTransferRunner {
                handle,
                socket_addr,
                ssh_private_key,
                user,
                transfer: FileTransfer::Download { source, destination },
            }
            .run(shutdown_signal)
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
