pub mod k8s;

use std::sync::LazyLock;

/// The default name for a pod created by Axon.
pub const DEFAULT_POD_NAME: &str = "axon";

/// The default container image used when creating a new pod if no other image
/// is specified.
pub const DEFAULT_IMAGE: &str = "docker.io/alpine:latest";

/// The default command and arguments for an interactive shell.
/// This typically points to a common shell executable like `/bin/sh`.
pub static DEFAULT_INTERACTIVE_SHELL: LazyLock<Vec<String>> =
    LazyLock::new(|| vec!["/bin/sh".to_string()]);
