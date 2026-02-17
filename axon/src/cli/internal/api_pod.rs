use std::time::Duration;

use k8s_openapi::api::core::v1::Pod;
use kube::{
    Api,
    runtime::{conditions::is_pod_running, wait::await_condition},
};
use snafu::ResultExt;

use crate::cli::{Error, error};

pub trait ApiPodExt {
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
