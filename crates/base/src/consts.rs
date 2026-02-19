use std::sync::LazyLock;

pub mod k8s {
    pub mod labels {
        pub const NAME: &str = "app.kubernetes.io/name";
        pub const VERSION: &str = "app.kubernetes.io/version";
        pub const MANAGED_BY: &str = "app.kubernetes.io/managed-by";
    }

    pub mod annotations {
        use std::sync::LazyLock;

        use crate::PROJECT_NAME;

        pub static SHELL_INTERACTIVE: LazyLock<String> =
            LazyLock::new(|| format!("{PROJECT_NAME}.shell/interactive"));

        pub static PORT_MAPPINGS_PREFIX: LazyLock<String> =
            LazyLock::new(|| format!("{PROJECT_NAME}.port-mappings"));

        pub static SERVICE_PORT_PREFIX: LazyLock<String> =
            LazyLock::new(|| format!("{PROJECT_NAME}.service-port"));

        pub static VERSION: LazyLock<String> = LazyLock::new(|| format!("{PROJECT_NAME}.version"));
    }
}

pub const DEFAULT_POD_NAME: &str = "axon";
pub const DEFAULT_IMAGE: &str = "docker.io/alpine:3.23";
pub static DEFAULT_INTERACTIVE_SHELL: LazyLock<Vec<String>> =
    LazyLock::new(|| vec!["/bin/sh".to_string()]);
