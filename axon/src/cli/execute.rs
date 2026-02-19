use std::time::Duration;

use clap::Args;
use k8s_openapi::api::core::v1::Pod;
use kube::Api;

use crate::{
    cli::{Error, internal::ApiPodExt},
    config::Config,
    pod_console::PodConsole,
};

#[derive(Args, Clone)]
pub struct ExecuteCommand {
    #[arg(short, long, help = "Namespace of the pod")]
    pub namespace: Option<String>,

    #[arg(short = 'p', long = "pod-name", help = "Name of the pod to attach to")]
    pub pod_name: Option<String>,

    #[arg(
        short = 't',
        long = "timeout-seconds",
        default_value = "15",
        help = "The maximum time in seconds to wait before timing out"
    )]
    pub timeout_secs: u64,

    #[arg(help = "Command to execute on the container", required = true)]
    pub command: Vec<String>,
}

impl ExecuteCommand {
    pub async fn run(self, kube_client: kube::Client, config: Config) -> Result<(), Error> {
        let Self { namespace, pod_name, command, timeout_secs } = self;

        // Resolve Identity
        let namespace = namespace
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| kube_client.default_namespace().to_string());

        let pod_name =
            pod_name.filter(|s| !s.is_empty()).unwrap_or_else(|| config.default_pod_name.clone());

        // Resolve Pod API & Status
        let api = Api::<Pod>::namespaced(kube_client, &namespace);
        let _pod = api
            .await_running_status(&pod_name, &namespace, Duration::from_secs(timeout_secs))
            .await?;

        PodConsole::new(api, pod_name, namespace, command).run().await.map_err(Error::from)
    }
}
