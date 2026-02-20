//! Handles the deletion of temporary Kubernetes pods managed by Axon.
//!
//! This module provides the `DeleteCommand` struct, which defines the
//! command-line arguments and logic for deleting one or more temporary pods. It
//! supports specifying pod names directly or using a fuzzy finder for
//! interactive selection if no names are provided.

use clap::{ArgAction, Args};
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::Pod;
use kube::{
    Api,
    api::{DeleteParams, ListParams},
};
use snafu::ResultExt;

use crate::{
    PROJECT_NAME,
    cli::{
        error::{self, Error},
        internal::{ResolvedResources, ResourceResolver},
    },
    config::Config,
    consts::k8s::labels,
    ui::fuzzy_finder::PodListExt as _,
};

/// Represents the command-line arguments for deleting temporary Kubernetes
/// pods.
///
/// This struct is used to parse the `delete` subcommand's arguments, allowing
/// users to specify the namespace and the names of pods to be deleted. If no
/// pod names are provided, an interactive fuzzy finder will be presented to
/// select pods managed by Axon.
#[derive(Args, Clone)]
pub struct DeleteCommand {
    /// Kubernetes namespace where the temporary pods are located.
    ///
    /// Defaults to the current Kubernetes context's namespace if not specified.
    #[arg(
        short,
        long,
        help = "Kubernetes namespace where the temporary pods are located. Defaults to the \
                current Kubernetes context's namespace."
    )]
    pub namespace: Option<String>,

    /// Names of the temporary pods to delete.
    ///
    /// If no names are provided, a fuzzy finder will be used to select pods
    /// managed by Axon.
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
    /// Executes the delete command, connecting to Kubernetes to remove
    /// specified pods.
    ///
    /// This function first resolves the target Kubernetes namespace. If no pod
    /// names are provided in the command, it lists all pods labeled as
    /// managed by Axon and uses an interactive fuzzy finder to allow the
    /// user to select which ones to delete. It then proceeds to delete the
    /// selected or specified pods.
    ///
    /// # Arguments
    ///
    /// * `self` - The `DeleteCommand` instance containing the parsed
    ///   command-line arguments.
    /// * `kube_client` - A `kube::Client` instance used to interact with the
    ///   Kubernetes API.
    /// * `config` - The application's `Config` instance.
    ///
    /// # Errors
    ///
    /// This function can return an `Error` in the following situations:
    ///
    /// * If the Kubernetes namespace cannot be resolved.
    /// * If listing pods fails (e.g., due to network issues or insufficient
    ///   permissions).
    /// * If the fuzzy finder encounters an error during interactive pod
    ///   selection.
    /// * If deleting a specific pod fails.
    ///
    /// # Panics
    ///
    /// This function does not explicitly panic, but underlying `kube` or
    /// `futures` operations might panic in extreme cases of unrecoverable
    /// errors (e.g., OOM).
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
                    println!("pod/{pod_name} deleted in namespace {namespace}");
                } else {
                    println!("pod/{pod_name} does not exist in namespace {namespace}");
                }

                Ok::<(), Error>(())
            }
        });
        let _unused =
            futures::stream::iter(futs).buffer_unordered(5).try_collect::<Vec<_>>().await?;

        Ok(())
    }
}
