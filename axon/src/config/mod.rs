mod error;
mod image_pull_policy;
mod log;
mod port_mapping;
mod service_ports;
mod spec;

use std::path::{Path, PathBuf};

use resolve_path::PathResolveExt;
use serde::{Deserialize, Serialize};
use snafu::ResultExt;

pub use self::{
    error::Error, image_pull_policy::ImagePullPolicy, log::LogConfig, port_mapping::PortMapping,
    service_ports::ServicePorts, spec::Spec,
};
use crate::{
    CLI_CONFIG_NAME, PROJECT_CONFIG_DIR, PROJECT_NAME, consts::DEFAULT_POD_NAME,
    fallback_project_config_directories,
};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    #[serde(default = "default_pod_name")]
    pub default_pod_name: String,

    #[serde(default = "default_spec")]
    pub default_spec: String,

    pub ssh_private_key_file_path: Option<PathBuf>,

    #[serde(default)]
    pub log: LogConfig,

    #[serde(default)]
    pub specs: Vec<Spec>,
}

impl Config {
    pub fn search_config_file_path() -> PathBuf {
        let paths = vec![Self::default_path()]
            .into_iter()
            .chain(fallback_project_config_directories().into_iter().map(|mut path| {
                path.push(CLI_CONFIG_NAME);
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
        [PROJECT_CONFIG_DIR.to_path_buf(), PathBuf::from(CLI_CONFIG_NAME)].into_iter().collect()
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

    pub fn template_basic() -> Vec<u8> { include_bytes!("templates/basic.yaml").to_vec() }
}

fn default_pod_name() -> String { DEFAULT_POD_NAME.to_string() }

fn default_spec() -> String { PROJECT_NAME.to_string() }

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn test_templates() {
        let _basic = serde_yaml::from_slice::<Config>(&Config::template_basic()).unwrap();
    }
}
