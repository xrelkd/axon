use std::time::Duration;

use clap::Args;
use k8s_openapi::api::core::v1::Pod;
use kube::Api;

use crate::{
    cli::{Error, internal::ApiPodExt},
    config::Config,
    ext::PodExt,
    pod_console::PodConsole,
};

#[derive(Args, Clone)]
pub struct AttachCommand {
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
        help = "Name of the temporary pod to attach to. If not specified, Axon's default pod name \
                will be used."
    )]
    pub pod_name: Option<String>,

    #[arg(
        short = 's',
        long = "shell",
        help = "Command and arguments for the interactive shell to use (e.g., `/bin/bash`, `bash \
                -c 'sh'`). If not specified, Axon will attempt to detect the shell."
    )]
    pub interactive_shell: Vec<String>,

    #[arg(
        short = 't',
        long = "timeout-seconds",
        default_value = "15",
        help = "The maximum time in seconds to wait for the pod to be running before timing out."
    )]
    pub timeout_secs: u64,
}

impl AttachCommand {
    pub async fn run(self, kube_client: kube::Client, config: Config) -> Result<(), Error> {
        let Self { namespace, pod_name, interactive_shell, timeout_secs } = self;

        // Resolve Identity
        let namespace = namespace
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| kube_client.default_namespace().to_string());

        let pod_name =
            pod_name.filter(|s| !s.is_empty()).unwrap_or_else(|| config.default_pod_name.clone());

        // Resolve Pod API & Status
        let api = Api::<Pod>::namespaced(kube_client, &namespace);
        let pod = api
            .await_running_status(&pod_name, &namespace, Duration::from_secs(timeout_secs))
            .await?;

        // Resolve Shell
        let shell =
            if interactive_shell.is_empty() { pod.interactive_shell() } else { interactive_shell };

        // Delegate behavior
        PodConsole::new(api, pod_name, namespace, shell).run().await.map_err(Error::from)
    }
}
