use serde::{Deserialize, Serialize};

use crate::{
    PROJECT_NAME,
    config::{ImagePullPolicy, PortMapping, ServicePorts},
    consts,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Spec {
    pub name: String,

    #[allow(clippy::struct_field_names)]
    pub image: String,

    #[allow(clippy::struct_field_names)]
    #[serde(default)]
    pub image_pull_policy: ImagePullPolicy,

    #[serde(default)]
    pub port_mappings: Vec<PortMapping>,

    #[serde(default)]
    pub service_ports: ServicePorts,

    #[serde(default)]
    pub command: Vec<String>,

    #[serde(default)]
    pub args: Vec<String>,

    #[serde(default)]
    pub interactive_shell: Vec<String>,
}

impl Default for Spec {
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
