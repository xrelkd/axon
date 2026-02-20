use std::{net::SocketAddr, time::Duration};

use clap::Args;
use k8s_openapi::api::core::v1::Pod;
use kube::Api;
use sigfinn::{ExitStatus, LifecycleManager};

use crate::{
    cli::{
        Error,
        internal::{ApiPodExt, ResolvedResources, ResourceResolver},
    },
    config::{Config, PortMapping},
    ext::PodExt,
    port_forwarder::PortForwarderBuilder,
};

#[derive(Args, Clone)]
pub struct PortForwardCommand {
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
        help = "Name of the temporary pod to forward ports for. If not specified, Axon's default \
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
}

impl PortForwardCommand {
    pub async fn run(self, kube_client: kube::Client, config: Config) -> Result<(), Error> {
        let Self { namespace, pod_name, timeout_secs } = self;

        // Resolve Identity
        let ResolvedResources { namespace, pod_name } =
            ResourceResolver::from((&kube_client, &config)).resolve(namespace, pod_name);

        let api = Api::<Pod>::namespaced(kube_client, &namespace);
        let port_mappings = api
            .await_running_status(&pod_name, &namespace, Duration::from_secs(timeout_secs))
            .await?
            .port_mappings();

        if port_mappings.is_empty() {
            return Ok(());
        }

        let lifecycle_manager = LifecycleManager::<Error>::new();

        for PortMapping { container_port, local_port, address } in port_mappings {
            let local_sock_addr = SocketAddr::new(address, local_port);
            let api = api.clone();
            let pod_name = pod_name.clone();
            let worker_name = format!("forwarder-{local_sock_addr}/{pod_name}:{container_port}");
            let create_fn = move |shutdown_signal| async move {
                let result = PortForwarderBuilder::new(api, pod_name, container_port)
                    .local_address(local_sock_addr)
                    .on_ready(|_| {})
                    .build()
                    .run(shutdown_signal)
                    .await;

                match result {
                    Ok(()) => ExitStatus::Success,
                    Err(err) => ExitStatus::Error(Error::from(err)),
                }
            };
            let _handle = lifecycle_manager.spawn(worker_name, create_fn);
        }

        tracing::info!("Forwarders started. Use Ctrl+C to stop.");

        if let Ok(Err(err)) = lifecycle_manager.serve().await {
            tracing::error!("{err}");
            Err(err)
        } else {
            Ok(())
        }
    }
}
