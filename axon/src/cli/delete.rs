use clap::{ArgAction, Args};
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::Pod;
use kube::{Api, api::DeleteParams};
use snafu::ResultExt;

use crate::cli::{error, error::Error};

#[derive(Args, Clone)]
pub struct DeleteCommand {
    #[arg(short, long, help = "Namespace to search for the pod")]
    pub namespace: Option<String>,

    #[arg(
        short = 'p',
        long = "pod-names",
        action = ArgAction::Append,
        required = true,
        num_args = 1..,
        help = "Pod names to delete"
    )]
    pub pod_names: Vec<String>,
}

impl DeleteCommand {
    pub async fn run(self, kube_client: kube::Client) -> Result<(), Error> {
        let Self { namespace, pod_names } = self;

        let namespace = namespace
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| kube_client.default_namespace().to_string());

        let api = Api::<Pod>::namespaced(kube_client, &namespace);
        let futs = pod_names.into_iter().map(|pod_name| {
            let api = api.clone();
            let namespace = namespace.clone();
            async move {
                let pod_exists = api.get(&pod_name).await.is_ok();
                if pod_exists {
                    let _resource = api.delete(&pod_name, &DeleteParams::default()).await.context(
                        error::DeletePodSnafu {
                            pod_name: pod_name.clone(),
                            namespace: namespace.clone(),
                        },
                    )?;
                    tracing::info!("pod/{pod_name} deleted in namespace {namespace}");
                } else {
                    tracing::info!("pod/{pod_name} does not exist in namespace {namespace}");
                }

                Ok::<(), Error>(())
            }
        });
        let _unused =
            futures::stream::iter(futs).buffer_unordered(5).try_collect::<Vec<_>>().await?;

        Ok(())
    }
}
