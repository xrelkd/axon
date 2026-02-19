use axon_base::{PROJECT_NAME, consts::k8s::labels};
use clap::Args;
use k8s_openapi::api::core::v1::Pod;
use kube::{Api, api::ListParams};
use snafu::ResultExt;
use tokio::io::AsyncWriteExt;

use crate::{
    cli::{error, error::Error},
    ui::table::PodListExt,
};

#[derive(Args, Clone)]
pub struct ListCommand {
    #[arg(short, long, help = "Namespace, use current namespace if not provided")]
    pub namespace: Option<String>,

    #[arg(short, long, help = "List all pods created by axon in all namespaces")]
    pub all_namespaces: bool,
}

impl ListCommand {
    pub async fn run(self, kube_client: kube::Client) -> Result<(), Error> {
        let Self { namespace, all_namespaces } = self;

        let namespace = namespace
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| kube_client.default_namespace().to_string());

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

        let _ = tokio::io::stdout().write_all(pods.render_table().as_bytes()).await.ok();
        let _ = tokio::io::stdout().write_u8(b'\n').await.ok();

        Ok(())
    }
}
