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
