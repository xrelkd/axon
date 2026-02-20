//! Provides the `setup` command for configuring SSH access to a running pod.

use std::{path::PathBuf, time::Duration};

use clap::Args;
use k8s_openapi::api::core::v1::Pod;
use kube::Api;

use crate::{
    cli::{
        Error,
        internal::{ApiPodExt, ResolvedResources, ResourceResolver},
        ssh::internal::Configurator,
    },
    config::Config,
    ssh,
};

/// Arguments for the `setup` command, used to configure SSH access to a
/// specified Kubernetes pod.
#[derive(Args, Clone)]
pub struct SetupCommand {
    /// Kubernetes namespace of the target pod. If not specified, the default
    /// namespace will be used.
    #[arg(
        short,
        long,
        help = "Kubernetes namespace of the target pod. If not specified, the default namespace \
                will be used."
    )]
    pub namespace: Option<String>,

    /// Name of the temporary pod to set up SSH for. If not specified, Axon's
    /// default pod name will be used.
    #[arg(
        short = 'p',
        long = "pod-name",
        help = "Name of the temporary pod to set up SSH for. If not specified, Axon's default pod \
                name will be used."
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

    /// Path to the SSH private key file whose corresponding public key will be
    /// authorized on the pod. If not specified, Axon will look for
    /// `sshPrivateKeyFilePath` in the configuration.
    #[arg(
        short = 'i',
        long = "ssh-private-key-file",
        help = "Path to the SSH private key file whose corresponding public key will be \
                authorized on the pod. If not specified, Axon will look for \
                `sshPrivateKeyFilePath` in the configuration."
    )]
    pub ssh_private_key_file: Option<PathBuf>,
}

impl SetupCommand {
    /// Executes the SSH setup process on the target Kubernetes pod.
    ///
    /// This function resolves the target pod's identity, loads the SSH key
    /// pair, waits for the pod to be in a running state, and then uploads
    /// the public SSH key to the pod to authorize access.
    ///
    /// # Arguments
    ///
    /// * `self` - The `SetupCommand` instance containing the command arguments.
    /// * `kube_client` - A `kube::Client` instance for interacting with the
    ///   Kubernetes API.
    /// * `config` - The application's `Config` instance.
    ///
    /// # Errors
    ///
    /// This function returns an `Err` variant of `crate::cli::Error` if:
    ///
    /// * The SSH private key file cannot be loaded or is invalid.
    /// * The target pod cannot be found or fails to reach a running state
    ///   within the specified timeout.
    /// * There's an issue communicating with the Kubernetes API.
    /// * The public SSH key cannot be uploaded to the pod.
    pub async fn run(self, kube_client: kube::Client, config: Config) -> Result<(), Error> {
        let Self { namespace, pod_name, timeout_secs, ssh_private_key_file } = self;

        // Resolve Identity
        let ResolvedResources { namespace, pod_name } =
            ResourceResolver::from((&kube_client, &config)).resolve(namespace, pod_name);

        let (_ssh_private_key, ssh_public_key) = ssh::resolve_ssh_key_pair(
            [ssh_private_key_file.as_ref(), config.ssh_private_key_file_path.as_ref()]
                .iter()
                .flatten(),
        )
        .await?;

        let api = Api::<Pod>::namespaced(kube_client, &namespace);
        let _unused = api
            .await_running_status(&pod_name, &namespace, Duration::from_secs(timeout_secs))
            .await?;

        Configurator::new(api, namespace, pod_name).upload_ssh_key(ssh_public_key).await
    }
}
