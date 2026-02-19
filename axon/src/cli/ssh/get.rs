use std::{path::PathBuf, time::Duration};

use clap::Args;
use k8s_openapi::api::core::v1::Pod;
use kube::Api;
use sigfinn::{ExitStatus, LifecycleManager};

use crate::{
    cli::{
        Error, error,
        internal::ApiPodExt,
        ssh::internal::{Configurator, DEFAULT_SSH_PORT, FileTransfer, FileTransferRunner},
    },
    config::Config,
    ext::PodExt,
    port_forwarder::PortForwarderBuilder,
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
                look for `ssh_private_key_file_path` in the configuration."
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

        let (ssh_private_key, ssh_public_key) = {
            let private_key = {
                let ((Some(private_key_file), _) | (None, Some(private_key_file))) =
                    (ssh_private_key_file, config.ssh_private_key_file_path)
                else {
                    return error::NoSshPrivateKeyProvidedSnafu.fail();
                };
                ssh::load_secret_key(private_key_file, None).await?
            };

            let public_key =
                private_key.public_key().to_openssh().expect("SSH public key should be valid");
            (private_key, public_key)
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

        Configurator::new(api.clone(), &namespace, &pod_name)
            .upload_ssh_key(ssh_public_key)
            .await?;

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
