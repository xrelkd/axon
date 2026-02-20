//! This module provides the `ListCommand` for listing Kubernetes pods managed
//! by Axon.

use clap::Args;
use k8s_openapi::api::core::v1::Pod;
use kube::{Api, api::ListParams};
use snafu::ResultExt;
use tokio::io::AsyncWriteExt;

use crate::{
    PROJECT_NAME,
    cli::{
        error::{self, Error},
        internal::{ResolvedResources, ResourceResolver},
    },
    config::Config,
    consts::k8s::labels,
    ui::table::PodListExt,
};

/// Represents the command to list Kubernetes pods managed by Axon.
///
/// This struct defines the command-line arguments available for listing pods.
#[derive(Args, Clone)]
pub struct ListCommand {
    #[arg(
        short,
        long,
        help = "Kubernetes namespace to list pods from. Defaults to the current Kubernetes \
                context's namespace."
    )]
    pub namespace: Option<String>,

    #[arg(
        short,
        long,
        help = "List all temporary pods created by Axon across all Kubernetes namespaces."
    )]
    pub all_namespaces: bool,
}

impl ListCommand {
    /// Executes the list command, fetching and displaying Kubernetes pods
    /// managed by Axon.
    ///
    /// This asynchronous function connects to the Kubernetes API, resolves the
    /// target namespace (if not specified, it uses the current context's
    /// namespace), and then lists pods that are labeled as managed by
    /// `PROJECT_NAME`. The results are then rendered to standard output in
    /// a tabular format.
    ///
    /// # Arguments
    ///
    /// * `self` - The `ListCommand` instance containing the command-line
    ///   arguments.
    /// * `kube_client` - A Kubernetes client instance used to interact with the
    ///   Kubernetes API.
    /// * `config` - The application configuration, potentially containing
    ///   default namespace information.
    ///
    /// # Errors
    ///
    /// This function returns an `Error` if any of the following occur:
    ///
    /// * Listing pods from the Kubernetes API fails (e.g., due to network
    ///   issues, authentication problems, or insufficient permissions).
    /// * Resolving the Kubernetes namespace fails.
    /// * Writing the output to `stdout` fails.
    pub async fn run(self, kube_client: kube::Client, config: Config) -> Result<(), Error> {
        let Self { namespace, all_namespaces } = self;

        // Resolve Identity
        let ResolvedResources { namespace, .. } =
            ResourceResolver::from((&kube_client, &config)).resolve(namespace, None);

        let list_params = ListParams {
            label_selector: Some(format!("{}={PROJECT_NAME}", labels::MANAGED_BY)),
            ..ListParams::default()
        };

        let pods = if all_namespaces {
            Api::<Pod>::all(kube_client).list(&list_params).await.context(error::ListPodsSnafu)?
        } else {
            Api::<Pod>::namespaced(kube_client, &namespace)
                .list(&list_params)
                .await
                .context(error::ListPodsWithNamespaceSnafu { namespace })?
        };

        let mut stdout = tokio::io::stdout();
        stdout.write_all(pods.render_table().as_bytes()).await.context(error::WriteStdoutSnafu)?;
        stdout.write_u8(b'\n').await.context(error::WriteStdoutSnafu)
    }
}
