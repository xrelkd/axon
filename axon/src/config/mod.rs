mod image_pull_policy;
mod port_mapping;

use std::{
    net::{IpAddr, Ipv4Addr},
    path::{Path, PathBuf},
};

use axon_base::{PROJECT_NAME, consts};
use resolve_path::PathResolveExt;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};

pub use self::{image_pull_policy::ImagePullPolicy, port_mapping::PortMapping};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default = "default_pod_name")]
    pub default_pod_name: String,

    #[serde(default = "default_spec")]
    pub default_spec: String,

    pub ssh_private_key_file_path: Option<PathBuf>,

    #[serde(default = "Vec::new")]
    pub specs: Vec<Spec>,

    #[serde(default = "axon_cli::config::LogConfig::default")]
    pub log: axon_cli::config::LogConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_pod_name: default_pod_name(),
            default_spec: default_spec(),
            ssh_private_key_file_path: None,
            specs: vec![Spec::spec_malm(), Spec::default()],
            log: axon_cli::config::LogConfig::default(),
        }
    }
}

impl Config {
    pub fn search_config_file_path() -> PathBuf {
        let paths = vec![Self::default_path()]
            .into_iter()
            .chain(axon_base::fallback_project_config_directories().into_iter().map(|mut path| {
                path.push(axon_base::CLI_CONFIG_NAME);
                path
            }))
            .collect::<Vec<_>>();
        for path in paths {
            let Ok(exists) = path.try_exists() else {
                continue;
            };
            if exists {
                return path;
            }
        }
        Self::default_path()
    }

    #[inline]
    pub fn default_path() -> PathBuf {
        [axon_base::PROJECT_CONFIG_DIR.to_path_buf(), PathBuf::from(axon_base::CLI_CONFIG_NAME)]
            .into_iter()
            .collect()
    }

    #[inline]
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let mut config: Self = {
            let path =
                path.as_ref().try_resolve().map(|path| path.to_path_buf()).with_context(|_| {
                    ResolveFilePathSnafu { file_path: path.as_ref().to_path_buf() }
                })?;
            let data = std::fs::read(&path).context(OpenConfigSnafu { filename: path.clone() })?;
            serde_yaml::from_slice(&data).context(ParseConfigSnafu { filename: path })?
        };

        config.log.file_path = match config.log.file_path.map(|path| {
            path.try_resolve()
                .map(|path| path.to_path_buf())
                .with_context(|_| ResolveFilePathSnafu { file_path: path.clone() })
        }) {
            Some(Ok(path)) => Some(path),
            Some(Err(err)) => return Err(err),
            None => None,
        };

        Ok(config)
    }

    pub fn find_default_spec(&self) -> Spec {
        self.specs.iter().find(|img| img.name == self.default_spec).cloned().unwrap_or_default()
    }

    pub fn find_spec_by_name(&self, name: &str) -> Option<Spec> {
        self.specs.iter().find(|img| img.name == name).cloned()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Spec {
    pub name: String,

    #[allow(clippy::struct_field_names)]
    pub image: String,

    #[allow(clippy::struct_field_names)]
    #[serde(default = "ImagePullPolicy::default")]
    pub image_pull_policy: ImagePullPolicy,

    #[serde(default = "Vec::new")]
    pub port_mappings: Vec<PortMapping>,

    #[serde(default = "Vec::new")]
    pub command: Vec<String>,

    #[serde(default = "Vec::new")]
    pub args: Vec<String>,

    #[serde(default = "Vec::new")]
    pub interactive_shell: Vec<String>,
}

impl Spec {
    fn spec_malm() -> Self {
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
            command: vec!["sh".to_string()],
            args: vec!["-c".to_string(), "while true; do sleep 1; done".to_string()],
            interactive_shell: vec!["/bin/sh".to_string()],
        }
    }
}

fn default_pod_name() -> String { PROJECT_NAME.to_string() }

fn default_spec() -> String { PROJECT_NAME.to_string() }

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Could not open config from {}, error: {source}", filename.display()))]
    OpenConfig { filename: PathBuf, source: std::io::Error },

    #[snafu(display("Count not parse config from {}, error: {source}", filename.display()))]
    ParseConfig { filename: PathBuf, source: serde_yaml::Error },

    #[snafu(display("Could not resolve file path {}, error: {source}", file_path.display()))]
    ResolveFilePath { file_path: PathBuf, source: std::io::Error },
}
