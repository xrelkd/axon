use k8s_openapi::{Metadata, api::core::v1::Pod};

use crate::{
    config::{PortMapping, ServicePorts},
    consts,
    consts::k8s::annotations,
};

pub trait PodExt {
    fn interactive_shell(&self) -> Vec<String>;

    fn port_mappings(&self) -> Vec<PortMapping>;

    fn service_ports(&self) -> ServicePorts;
}

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
