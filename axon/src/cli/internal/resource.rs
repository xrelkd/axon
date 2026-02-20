use crate::config::Config;

/// A struct responsible for resolving Kubernetes resource names,
/// typically a namespace and a pod name, using a Kubernetes client
/// and application configuration for defaults.
pub struct ResourceResolver<'k, 'c> {
    kube_client: &'k kube::Client,
    config: &'c Config,
}

/// Contains the resolved namespace and pod name for a Kubernetes resource.
pub struct ResolvedResources {
    /// The Kubernetes namespace.
    pub namespace: String,
    /// The name of the Kubernetes pod.
    pub pod_name: String,
}

impl<'k, 'c> From<(&'k kube::Client, &'c Config)> for ResourceResolver<'k, 'c> {
    /// Creates a new [`ResourceResolver`] instance from a Kubernetes client and
    /// an application configuration.
    ///
    /// # Arguments
    ///
    /// * `(kube_client, config)` - A tuple containing a reference to a
    ///   Kubernetes client and a reference to the application's configuration.
    fn from((kube_client, config): (&'k kube::Client, &'c Config)) -> Self {
        Self { kube_client, config }
    }
}

impl ResourceResolver<'_, '_> {
    /// Resolves the Kubernetes namespace and pod name.
    ///
    /// If the provided `namespace` or `pod_name` are `None` or empty,
    /// this method falls back to the default namespace from the Kubernetes
    /// client or the default pod name from the application configuration,
    /// respectively.
    ///
    /// # Arguments
    ///
    /// * `namespace` - An optional `String` representing the desired Kubernetes
    ///   namespace. If `None` or empty, the Kubernetes client's default
    ///   namespace is used.
    /// * `pod_name` - An optional `String` representing the desired pod name.
    ///   If `None` or empty, the application's default pod name is used.
    ///
    /// # Returns
    ///
    /// A [`ResolvedResources`] struct containing the determined namespace and
    /// pod name.
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
