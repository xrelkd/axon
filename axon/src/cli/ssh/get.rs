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

#[derive(Args, Clone)]
pub struct GetCommand {
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
        help = "Name of the temporary pod to get the file from. If not specified, Axon's default \
                pod name will be used."
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

    #[arg(help = "Path to the file on the remote pod to download.")]
    source: PathBuf,

    #[arg(help = "Local path where the downloaded file will be saved.")]
    destination: PathBuf,
}

impl GetCommand {
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

        let (ssh_private_key, ssh_public_key) =
            ssh::load_ssh_key_pair(ssh_private_key_file, config.ssh_private_key_file_path).await?;

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
