//! Axon-specific Kubernetes definitions.

pub mod labels {
    //! Kubernetes labels used by Axon.

    /// The `app.kubernetes.io/managed-by` label value, indicating that a
    /// resource is managed by Axon.
    pub const MANAGED_BY: &str = "app.kubernetes.io/managed-by";

    /// The `kubectl.kubernetes.io/default-container` annotation, specifying
    /// the default container to attach to in a multi-container pod.
    pub const DEFAULT_CONTAINER: &str = "kubectl.kubernetes.io/default-container";
}

pub mod annotations {
    //! Kubernetes annotations used by Axon.

    use std::sync::LazyLock;

    use crate::PROJECT_NAME;

    /// The annotation key used to indicate whether a shell session should
    /// be interactive. This is typically used on pods to configure
    /// shell behavior.
    pub static SHELL_INTERACTIVE: LazyLock<String> =
        LazyLock::new(|| format!("{PROJECT_NAME}.shell/interactive"));

    /// The prefix for annotations used to define port mappings for a pod.
    /// Specific port mapping annotations will follow this prefix.
    pub static PORT_MAPPINGS_PREFIX: LazyLock<String> =
        LazyLock::new(|| format!("{PROJECT_NAME}.port-mappings"));

    /// The prefix for annotations used to define service port
    /// configurations for a pod. Specific service port annotations
    /// will follow this prefix.
    pub static SERVICE_PORT_PREFIX: LazyLock<String> =
        LazyLock::new(|| format!("{PROJECT_NAME}.service-port"));

    /// The annotation key used to store the version of Axon that created or
    /// last modified a resource.
    pub static VERSION: LazyLock<String> = LazyLock::new(|| format!("{PROJECT_NAME}.version"));
}
