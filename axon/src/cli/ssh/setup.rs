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

#[derive(Args, Clone)]
pub struct SetupCommand {
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
        help = "Name of the temporary pod to set up SSH for. If not specified, Axon's default pod \
                name will be used."
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
        help = "Path to the SSH private key file whose corresponding public key will be \
                authorized on the pod. If not specified, Axon will look for \
                `sshPrivateKeyFilePath` in the configuration."
    )]
    pub ssh_private_key_file: Option<PathBuf>,
}

impl SetupCommand {
    pub async fn run(self, kube_client: kube::Client, config: Config) -> Result<(), Error> {
        let Self { namespace, pod_name, timeout_secs, ssh_private_key_file } = self;

        // Resolve Identity
        let ResolvedResources { namespace, pod_name } =
            ResourceResolver::from((&kube_client, &config)).resolve(namespace, pod_name);

        let (_, ssh_public_key) =
            ssh::load_ssh_key_pair(ssh_private_key_file, config.ssh_private_key_file_path).await?;

        let api = Api::<Pod>::namespaced(kube_client, &namespace);
        let _unused = api
            .await_running_status(&pod_name, &namespace, Duration::from_secs(timeout_secs))
            .await?;

        Configurator::new(api, namespace, pod_name).upload_ssh_key(ssh_public_key).await
    }
}
