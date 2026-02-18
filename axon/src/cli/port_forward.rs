use std::{collections::HashMap, net::SocketAddr, time::Duration};

use clap::Args;
use k8s_openapi::{Metadata, api::core::v1::Pod};
use kube::Api;
use sigfinn::LifecycleManager;

use crate::{
    config::{Config, PortMapping},
    error::Error,
    ext::ApiPodExt,
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

        let pods = Api::<Pod>::namespaced(kube_client, &namespace);

        let port_map = pods
            .await_running_status(&pod_name, &namespace, Duration::from_secs(timeout_secs))
            .await?
            .map_or_else(HashMap::new, |pod| {
                pod.metadata()
                    .annotations
                    .iter()
                    .flatten()
                    .filter_map(|(key, value)| {
                        let PortMapping { container_port, local_port, address } =
                            PortMapping::try_from_kubernetes_annotation(key, value).ok()?;
                        Some((SocketAddr::new(address, local_port), container_port))
                    })
                    .collect()
            });

        let lifecycle_manager = LifecycleManager::<Error>::new();

        for (local_socket_address, remote_port) in port_map {
            let pods = pods.clone();
            let pod_name = pod_name.clone();
            let handle = lifecycle_manager.handle();
            let worker_name = format!("forwarder-{local_socket_address}/{pod_name}:{remote_port}");
            let create_fn = move |shutdown_signal| async move {
                pods.port_forward(
                    &pod_name,
                    local_socket_address,
                    remote_port,
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
