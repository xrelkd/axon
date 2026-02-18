use std::{net::SocketAddr, time::Duration};

use clap::Args;
use k8s_openapi::api::core::v1::Pod;
use kube::Api;
use sigfinn::LifecycleManager;

use crate::{
    config::{Config, PortMapping},
    error::Error,
    ext::{ApiPodExt, PodExt},
};

#[derive(Args, Clone)]
pub struct PortForwardCommand {
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
}

impl PortForwardCommand {
    pub async fn run(self, kube_client: kube::Client, config: Config) -> Result<(), Error> {
        let Self { namespace, pod_name, timeout_secs } = self;

        let namespace = namespace
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| kube_client.default_namespace().to_string());
        let pod_name =
            pod_name.filter(|s| !s.is_empty()).unwrap_or_else(|| config.default_pod_name.clone());

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
            let handle = lifecycle_manager.handle();
            let worker_name = format!("forwarder-{local_sock_addr}/{pod_name}:{container_port}");
            let create_fn = move |shutdown_signal| async move {
                api.port_forward(
                    &pod_name,
                    local_sock_addr,
                    container_port,
                    handle,
                    shutdown_signal,
                    |_| {},
                )
                .await
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
