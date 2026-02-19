use axon_base::{PROJECT_NAME, consts::k8s::labels};
use clap::{ArgAction, Args};
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::Pod;
use kube::{
    Api,
    api::{DeleteParams, ListParams},
};
use snafu::ResultExt;

use crate::{
    cli::{
        error::{self, Error},
        internal::{ResolvedResources, ResourceResolver},
    },
    config::Config,
    ui::fuzzy_finder::PodListExt as _,
};

#[derive(Args, Clone)]
pub struct DeleteCommand {
    #[arg(
        short,
        long,
        help = "Kubernetes namespace where the temporary pods are located. Defaults to the \
                current Kubernetes context's namespace."
    )]
    pub namespace: Option<String>,

    #[arg(
        short = 'p',
        long = "pod-names",
        action = ArgAction::Append,
        num_args = 1..,
        help = "Names of the temporary pods to delete. If no names are provided, a fuzzy finder will be used to select pods managed by Axon."
    )]
    pub pod_names: Vec<String>,
}

impl DeleteCommand {
    pub async fn run(self, kube_client: kube::Client, config: Config) -> Result<(), Error> {
        let Self { namespace, pod_names } = self;

        // Resolve Identity
        let ResolvedResources { namespace, .. } =
            ResourceResolver::from((&kube_client, &config)).resolve(namespace, None);

        let api = Api::<Pod>::namespaced(kube_client, &namespace);
        let pod_names = if pod_names.is_empty() {
            let list_params = ListParams {
                label_selector: Some(format!("{}={PROJECT_NAME}", labels::MANAGED_BY)),
                ..ListParams::default()
            };

            api.list(&list_params)
                .await
                .with_context(|_| error::ListPodsWithNamespaceSnafu {
                    namespace: namespace.clone(),
                })?
                .find_pod_names()
                .await
        } else {
            pod_names
        };

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
