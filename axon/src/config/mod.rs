mod error;
mod image_pull_policy;
mod port_mapping;
mod service_ports;
mod spec;

use std::path::{Path, PathBuf};

use axon_base::PROJECT_NAME;
use resolve_path::PathResolveExt;
use serde::{Deserialize, Serialize};
use snafu::ResultExt;

pub use self::{
    error::Error, image_pull_policy::ImagePullPolicy, port_mapping::PortMapping,
    service_ports::ServicePorts, spec::Spec,
};

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
                    error::ResolveFilePathSnafu { file_path: path.as_ref().to_path_buf() }
                })?;
            let data =
                std::fs::read(&path).context(error::OpenConfigSnafu { filename: path.clone() })?;
            serde_yaml::from_slice(&data).context(error::ParseConfigSnafu { filename: path })?
        };

        config.log.file_path = match config.log.file_path.map(|path| {
            path.try_resolve()
                .map(|path| path.to_path_buf())
                .with_context(|_| error::ResolveFilePathSnafu { file_path: path.clone() })
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

fn default_pod_name() -> String { PROJECT_NAME.to_string() }

fn default_spec() -> String { PROJECT_NAME.to_string() }
