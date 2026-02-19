use crate::config::Config;

pub struct ResourceResolver<'k, 'c> {
    kube_client: &'k kube::Client,
    config: &'c Config,
}

pub struct ResolvedResources {
    pub namespace: String,
    pub pod_name: String,
}

impl<'k, 'c> From<(&'k kube::Client, &'c Config)> for ResourceResolver<'k, 'c> {
    fn from((kube_client, config): (&'k kube::Client, &'c Config)) -> Self {
        Self { kube_client, config }
    }
}

impl ResourceResolver<'_, '_> {
    pub fn resolve(
        &self,
        namespace: Option<String>,
        pod_name: Option<String>,
    ) -> ResolvedResources {
        let Self { kube_client, config } = self;
        let namespace = namespace
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| kube_client.default_namespace().to_string());
        let pod_name =
            pod_name.filter(|s| !s.is_empty()).unwrap_or_else(|| config.default_pod_name.clone());

        ResolvedResources { namespace, pod_name }
    }
}
