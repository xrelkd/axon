/// This module provides extensions for the Kubernetes `Api<Pod>` type.
use std::time::Duration;

use k8s_openapi::api::core::v1::Pod;
use kube::{
    Api,
    runtime::{conditions::is_pod_running, wait::await_condition},
};
use snafu::ResultExt;

use crate::cli::{Error, error};

/// Extension trait for `kube::Api<Pod>` providing additional utility methods.
pub trait ApiPodExt {
    /// Asynchronously waits for a specific Pod to reach a running status.
    ///
    /// This method uses a timeout to prevent indefinite waiting. If the Pod
    /// does not transition to a running state within the specified duration,
    /// an error is returned.
    ///
    /// # Arguments
    ///
    /// * `pod_name` - The name of the Pod to wait for.
    /// * `namespace` - The namespace where the Pod resides.
    /// * `timeout` - The maximum duration to wait for the Pod to become
    ///   running.
    ///
    /// # Returns
    ///
    /// A `Result` which is `Ok(Pod)` if the Pod becomes running within the
    /// timeout, or an `Err` if a timeout occurs or other Kubernetes API
    /// errors happen.
    ///
    /// # Errors
    ///
    /// Returns `Error::WaitForPodStatus` if the timeout is reached before the
    /// Pod enters a running state.
    /// Returns `error::GetPodStatusSnafu` if there's an issue checking the
    /// Pod's status or if the Pod is not found.
    /// Returns `error::GetPodSnafu` if a direct `get` call to the Kubernetes
    /// API fails after a timeout or status check issue.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::time::Duration;
    /// use kube::{Api, Client};
    /// use k8s_openapi::api::core::v1::Pod;
    /// use axon::cli::{Error, error};
    /// use axon::cli::internal::api_pod::ApiPodExt; // Assuming this is the correct path
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Error> {
    ///     let client = Client::try_default().await.map_err(|e| error::KubeClientSnafu { source: e }.build())?;
    ///     let pods: Api<Pod> = Api::namespaced(client, "default");
    ///
    ///     let pod_name = "my-app-pod";
    ///     let namespace = "default";
    ///     let timeout = Duration::from_secs(60);
    ///
    ///     match pods.await_running_status(pod_name, namespace, timeout).await {
    ///         Ok(pod) => println!("Pod {} is running!", pod.metadata.and_then(|m| m.name).unwrap_or_default()),
    ///         Err(e) => eprintln!("Failed to wait for pod {}: {}", pod_name, e),
    ///     }
    ///     Ok(())
    /// }
    /// ```
    async fn await_running_status(
        &self,
        pod_name: &str,
        namespace: &str,
        timeout: Duration,
    ) -> Result<Pod, Error>;
}

impl ApiPodExt for Api<Pod> {
    async fn await_running_status(
        &self,
        pod_name: &str,
        namespace: &str,
        timeout: Duration,
    ) -> Result<Pod, Error> {
        // Wait until the pod is running, otherwise we get 500 error.
        let maybe_pod = tokio::time::timeout(
            timeout,
            await_condition(self.clone(), pod_name, is_pod_running()),
        )
        .await
        .map_err(|_| Error::WaitForPodStatus {
            namespace: namespace.to_string(),
            pod_name: pod_name.to_string(),
        })?
        .with_context(|_| error::GetPodStatusSnafu {
            namespace: namespace.to_string(),
            pod_name: pod_name.to_string(),
        })?;
        match maybe_pod {
            Some(pod) => Ok(pod),
            None => self.get(pod_name).await.with_context(|_| error::GetPodSnafu {
                namespace: namespace.to_string(),
                pod_name: pod_name.to_string(),
            }),
        }
    }
}
