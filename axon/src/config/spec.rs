//! Defines the `Spec` structure, which describes the configuration for a
//! container or service managed by the application.
//!
//! This module provides the `Spec` struct, used for deserializing and
//! serializing configuration related to container deployment, including image,
//! command, arguments, port mappings, and interactive shell settings.

use serde::{Deserialize, Serialize};

use crate::{
    PROJECT_NAME,
    config::{ImagePullPolicy, PortMapping, ServicePorts},
    consts,
};

/// Represents the specification for a container or service.
///
/// This struct holds all the necessary configuration details for running a
/// container, including its name, image, image pull policy, port mappings,
/// service ports, command, arguments, and interactive shell settings.
///
/// It is designed to be deserialized from and serialized to various formats,
/// with field names automatically converted to `camelCase`.
///
/// # Fields
///
/// - `name`: The name of the container or service.
/// - `image`: The Docker image to use for the container.
/// - `image_pull_policy`: Defines when the Docker image should be pulled.
/// - `port_mappings`: A list of port mappings from the host to the container.
/// - `service_ports`: Configuration for service ports exposed by the container.
/// - `command`: The command to execute inside the container.
/// - `args`: Additional arguments to pass to the command.
/// - `interactive_shell`: The command to use for an interactive shell session.
///
/// # Examples
///
/// ```rust
/// use crate::config::{ImagePullPolicy, PortMapping, ServicePorts, Spec};
///
/// let spec = Spec {
///     name: "my-custom-container".to_string(),
///     image: "ubuntu:latest".to_string(),
///     image_pull_policy: ImagePullPolicy::IfNotPresent,
///     port_mappings: vec![
///         PortMapping {
///             host_port: 8080,
///             container_port: 80,
///         },
///     ],
///     service_ports: ServicePorts::default(),
///     command: vec!["bash".to_string()],
///     args: vec!["-c".to_string(), "echo Hello World!".to_string()],
///     interactive_shell: vec!["/bin/bash".to_string()],
/// };
///
/// assert_eq!(spec.name, "my-custom-container");
/// assert_eq!(spec.image, "ubuntu:latest");
/// assert_eq!(spec.command, vec!["bash".to_string()]);
/// ```
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Spec {
    /// The name of the container or service.
    pub name: String,

    /// The Docker image to use for the container (e.g., "ubuntu:latest",
    /// "my-repo/my-image:1.0").
    #[allow(clippy::struct_field_names)]
    pub image: String,

    /// Defines when the Docker image should be pulled.
    ///
    /// Defaults to `ImagePullPolicy::Always` if not specified.
    #[allow(clippy::struct_field_names)]
    #[serde(default)]
    pub image_pull_policy: ImagePullPolicy,

    /// A list of port mappings from the host to the container.
    ///
    /// Each `PortMapping` specifies a `host_port` and a `container_port`.
    /// Defaults to an empty list.
    #[serde(default)]
    pub port_mappings: Vec<PortMapping>,

    /// Configuration for service ports exposed by the container.
    #[serde(default)]
    pub service_ports: ServicePorts,

    /// The command to execute inside the container.
    #[serde(default)]
    pub command: Vec<String>,

    /// Additional arguments to pass to the command.
    #[serde(default)]
    pub args: Vec<String>,

    /// The command to use for an interactive shell session.
    #[serde(default)]
    pub interactive_shell: Vec<String>,
}

impl Default for Spec {
    /// Creates a default `Spec` instance.
    ///
    /// The default specification includes:
    /// - `name`: The project's name (`PROJECT_NAME`).
    /// - `image`: The default image (`consts::DEFAULT_IMAGE`).
    /// - `image_pull_policy`: `ImagePullPolicy::default()` (typically `Always`
    ///   or `IfNotPresent`).
    /// - `port_mappings`: An empty vector.
    /// - `service_ports`: `ServicePorts::default()`.
    /// - `command`: `["sh"]`.
    /// - `args`: `["-c", "while true; do sleep 1; done"]` to keep the container
    ///   running indefinitely.
    /// - `interactive_shell`: `["/bin/sh"]`.
    ///
    /// # Returns
    ///
    /// A new `Spec` instance populated with default values.
    fn default() -> Self {
        Self {
            name: PROJECT_NAME.to_string(),
            image: consts::DEFAULT_IMAGE.to_string(),
            image_pull_policy: ImagePullPolicy::default(),
            port_mappings: Vec::new(),
            service_ports: ServicePorts::default(),
            command: vec!["sh".to_string()],
            args: vec!["-c".to_string(), "while true; do sleep 1; done".to_string()],
            interactive_shell: vec!["/bin/sh".to_string()],
        }
    }
}
