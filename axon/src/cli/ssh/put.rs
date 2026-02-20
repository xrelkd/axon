//! Provides the `PutCommand` struct for uploading files to a Kubernetes pod via
//! SSH.
//!
//! This module defines the command-line arguments and logic required to
//! facilitate secure file transfer from a local machine to a remote pod within
//! a Kubernetes cluster, leveraging SSH. It handles pod resolution, SSH key
//! management, port forwarding, and the actual file transfer operation.

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

/// Represents the command-line arguments for the `put` operation.
///
/// This struct defines the various options available when using the `axon put`
/// command to upload a file to a specified Kubernetes pod. It includes options
/// for targeting the pod, configuring SSH, and specifying file paths.
#[derive(Args, Clone)]
pub struct PutCommand {
    #[arg(
        short,
        long,
        help = "Kubernetes namespace of the target pod. If not specified, the default namespace \
                will be used."
    )]
    pub namespace: Option<String>,

    #[arg(
        short = 'p',
        long = "pod-name",
        help = "Name of the temporary pod to upload the file to. If not specified, Axon's default \
                pod name will be used."
    )]
    pub pod_name: Option<String>,

    #[arg(
        short = 't',
        long = "timeout-seconds",
        default_value = "15",
        help = "The maximum time in seconds to wait for the pod to be running before timing out."
    )]
    pub timeout_secs: u64,

    #[arg(
        short = 'i',
        long = "ssh-private-key-file",
        help = "Path to the SSH private key file for authentication. If not specified, Axon will \
                look for `sshPrivateKeyFilePath` in the configuration."
    )]
    pub ssh_private_key_file: Option<PathBuf>,

    #[arg(
        short = 'u',
        long = "user",
        default_value = "root",
        help = "User name to connect as via SSH on the remote pod."
    )]
    pub user: String,

    #[arg(help = "Local path to the file to upload.")]
    pub source: PathBuf,

    #[arg(help = "Path on the remote pod where the file will be saved.")]
    pub destination: PathBuf,
}

impl PutCommand {
    /// Executes the file upload operation to a Kubernetes pod.
    ///
    /// This asynchronous function resolves the target pod, loads SSH keys, sets
    /// up port forwarding, uploads the SSH public key to the pod, and then
    /// transfers the specified local file to the remote destination on the
    /// pod using SSH. It manages the lifecycle of the SSH client and
    /// port-forwarding processes.
    ///
    /// # Arguments
    ///
    /// * `self` - The `PutCommand` instance containing all command-line
    ///   arguments.
    /// * `kube_client` - A Kubernetes client used to interact with the API
    ///   server.
    /// * `config` - The application's configuration, potentially containing
    ///   default values for various settings.
    ///
    /// # Errors
    ///
    /// This function can return an `Error` in several scenarios, including:
    ///
    /// * If SSH private key loading fails (e.g., file not found, invalid
    ///   format).
    /// * If the target pod cannot be found or does not reach a running status
    ///   within the timeout.
    /// * If the SSH public key cannot be uploaded to the pod (e.g., due to
    ///   permissions or pod issues).
    /// * If port forwarding fails to set up.
    /// * If the SSH file transfer operation encounters an error (e.g.,
    ///   connection issues, permission denied on the remote host, file system
    ///   errors).
    /// * If the SSH local socket address receiver fails to provide an address.
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
                transfer: FileTransfer::Upload { source, destination },
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
