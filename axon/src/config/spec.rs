use std::net::{IpAddr, Ipv4Addr};

use axon_base::{PROJECT_NAME, consts};
use serde::{Deserialize, Serialize};

use crate::config::{ImagePullPolicy, PortMapping, ServicePorts};

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

impl Spec {
    pub fn spec_malm() -> Self {
        Self {
            name: "malm".to_string(),
            image: "ghcr.io/xrelkd/malm:0.7.0".to_string(),
            image_pull_policy: ImagePullPolicy::IfNotPresent,
            port_mappings: vec![
                PortMapping {
                    container_port: 8080,
                    local_port: 8080,
                    address: IpAddr::V4(Ipv4Addr::LOCALHOST),
                },
                PortMapping {
                    container_port: 22,
                    local_port: 22222,
                    address: IpAddr::V4(Ipv4Addr::LOCALHOST),
                },
            ],
            service_ports: ServicePorts { ssh: Some(22), http: Some(8080), https: None },
            command: Vec::new(),
            args: Vec::new(),
            interactive_shell: vec!["/bin/zsh".to_string()],
        }
    }
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
