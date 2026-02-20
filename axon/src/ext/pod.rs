use k8s_openapi::{Metadata, api::core::v1::Pod};

use crate::{
    config::{PortMapping, ServicePorts},
    consts,
    consts::k8s::annotations,
};

/// Extension trait for `Pod` providing methods for extracting Axon-specific
/// configurations.
///
/// This trait adds functionality to Kubernetes `Pod` objects to easily retrieve
/// configurations relevant to the Axon application, such as interactive shell
/// commands, port mappings, and service port configurations.
pub trait PodExt {
    /// Determines the interactive shell command for the pod.
    ///
    /// This method checks for a specific Kubernetes annotation on the pod. If
    /// the annotation is found and contains valid JSON representing a list
    /// of strings, those strings are used as the interactive shell command.
    /// Otherwise, a default interactive shell command defined in
    /// `consts::DEFAULT_INTERACTIVE_SHELL` is used.
    ///
    /// # Returns
    ///
    /// A `Vec<String>` representing the interactive shell command and its
    /// arguments.
    fn interactive_shell(&self) -> Vec<String>;

    /// Extracts Axon-specific port mappings from the pod's annotations.
    ///
    /// This method iterates through the pod's annotations and attempts to parse
    /// `PortMapping` objects from them using
    /// `PortMapping::try_from_kubernetes_annotation`. Only successfully
    /// parsed port mappings are collected.
    ///
    /// # Returns
    ///
    /// A `Vec<PortMapping>` containing all valid Axon-specific port mappings
    /// found in the pod's annotations. This vector will be empty if no such
    /// annotations are found or if parsing fails for all.
    fn port_mappings(&self) -> Vec<PortMapping>;

    /// Extracts Axon-specific service port configurations from the pod's
    /// annotations.
    ///
    /// This method delegates to `ServicePorts::from_kubernetes_annotations` to
    /// process the pod's annotations and construct a `ServicePorts` object.
    /// This allows for a centralized way to define and retrieve service
    /// port details from Kubernetes metadata.
    ///
    /// # Returns
    ///
    /// A `ServicePorts` object representing the pod's configured service ports.
    /// This object will reflect any service port annotations found on the pod.
    fn service_ports(&self) -> ServicePorts;
}

/// Implements the `PodExt` trait for `k8s_openapi::api::core::v1::Pod`,
/// providing convenience methods to access Axon-specific pod configurations.
impl PodExt for Pod {
    fn interactive_shell(&self) -> Vec<String> {
        if let Some(annotations) = &self.metadata().annotations
            && let Some(shell_json) = annotations.get(annotations::SHELL_INTERACTIVE.as_str())
            && let Ok(shell) = serde_json::from_str::<Vec<String>>(shell_json)
            && !shell.is_empty()
        {
            shell
        } else {
            consts::DEFAULT_INTERACTIVE_SHELL.clone()
        }
    }

    fn port_mappings(&self) -> Vec<PortMapping> {
        self.metadata()
            .annotations
            .iter()
            .flatten()
            .filter_map(|(key, value)| PortMapping::try_from_kubernetes_annotation(key, value).ok())
            .collect()
    }

    fn service_ports(&self) -> ServicePorts {
        ServicePorts::from_kubernetes_annotations(self.metadata().annotations.iter().flatten())
    }
}
