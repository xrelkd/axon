//! Configuration management for the Axon CLI.
//!
//! This module handles loading, parsing, and managing application
//! configuration, including default pod names, specifications, SSH keys, and
//! logging settings. It also provides utilities to locate the configuration
//! file and retrieve specific specifications.

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

/// Represents the top-level structure of the application's configuration.
///
/// This struct holds various settings such as the default pod name,
/// default specification, SSH private key path, logging configuration,
/// and a list of defined specifications.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    /// The default name to use for new pods if not explicitly specified.
    #[serde(default = "default_pod_name")]
    pub default_pod_name: String,

    /// The name of the default `Spec` to use from the `specs` list.
    #[serde(default = "default_spec")]
    pub default_spec: String,

    /// An optional path to the SSH private key file to be used for connections.
    pub ssh_private_key_file_path: Option<PathBuf>,

    /// Configuration for application logging.
    #[serde(default)]
    pub log: LogConfig,

    /// A list of available specifications (`Spec`) that define different pod
    /// configurations.
    #[serde(default)]
    pub specs: Vec<Spec>,
}

impl Config {
    /// Searches for the application configuration file in various predefined
    /// locations.
    ///
    /// It first checks the default path (`default_path()`) and then
    /// falls back to other project configuration directories.
    ///
    /// # Returns
    ///
    /// A `PathBuf` representing the first found configuration file. If no
    /// configuration file is found, it returns the `default_path()`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::path::PathBuf;
    /// use axon::config::Config;
    ///
    /// // Assuming a config file exists at one of the search paths
    /// let config_path: PathBuf = Config::search_config_file_path();
    /// println!("Found config at: {:?}", config_path);
    /// ```
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

    /// Returns the default path for the application's configuration file.
    ///
    /// This path is typically derived from `PROJECT_CONFIG_DIR` and
    /// `CLI_CONFIG_NAME`.
    ///
    /// # Returns
    ///
    /// A `PathBuf` indicating the default location of the configuration file.
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::path::PathBuf;
    /// use axon::config::Config;
    ///
    /// let default_cfg_path: PathBuf = Config::default_path();
    /// println!("Default config path: {:?}", default_cfg_path);
    /// ```
    #[inline]
    pub fn default_path() -> PathBuf {
        [PROJECT_CONFIG_DIR.to_path_buf(), PathBuf::from(CLI_CONFIG_NAME)].into_iter().collect()
    }

    /// Loads and parses the application configuration from the specified path.
    ///
    /// This function reads a YAML configuration file, deserializes it into a
    /// `Config` struct, and resolves any relative paths within the
    /// configuration.
    ///
    /// # Arguments
    ///
    /// * `path` - The path to the configuration file.
    ///
    /// # Returns
    ///
    /// A `Result` which is `Ok` with the loaded `Config` on success, or an
    /// `Err` containing an `Error` if the file cannot be read, parsed, or
    /// paths resolved.
    ///
    /// # Errors
    ///
    /// This function can return an `Error` in the following cases:
    ///
    /// * `ResolveFilePathSnafu`: If a path (e.g., `ssh_private_key_file_path`
    ///   or `log.file_path`) cannot be resolved to an absolute path.
    /// * `OpenConfigSnafu`: If the configuration file cannot be opened or read.
    /// * `ParseConfigSnafu`: If the content of the configuration file is not
    ///   valid YAML or does not conform to the `Config` struct's expected
    ///   structure.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::path::PathBuf;
    /// use axon::config::Config;
    ///
    /// let config_path = PathBuf::from("path/to/axon_config.yaml");
    /// match Config::load(config_path) {
    ///     Ok(config) => println!("Configuration loaded successfully."),
    ///     Err(e) => eprintln!("Failed to load configuration: {}", e),
    /// }
    /// ```
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

        let try_resolve_path = |path: Option<&PathBuf>| -> Result<Option<PathBuf>, Error> {
            match path.map(|path| {
                path.try_resolve()
                    .map(|path| path.to_path_buf())
                    .with_context(|_| error::ResolveFilePathSnafu { file_path: path.clone() })
            }) {
                Some(Ok(path)) => Ok(Some(path)),
                Some(Err(err)) => Err(err),
                None => Ok(None),
            }
        };

        config.ssh_private_key_file_path =
            try_resolve_path(config.ssh_private_key_file_path.as_ref())?;
        config.log.file_path = try_resolve_path(config.log.file_path.as_ref())?;

        Ok(config)
    }

    /// Finds and returns the default `Spec` based on the `default_spec` field.
    ///
    /// If a `Spec` with a matching name is found in the `specs` list, it is
    /// returned. Otherwise, a default-constructed `Spec` is returned.
    ///
    /// # Returns
    ///
    /// The `Spec` designated as the default, or a newly created default `Spec`
    /// if the named default is not found.
    ///
    /// # Example
    ///
    /// ```rust
    /// use axon::config::{Config, Spec};
    ///
    /// let mut config = Config {
    ///     default_pod_name: "my-pod".to_string(),
    ///     default_spec: "custom-spec".to_string(),
    ///     ssh_private_key_file_path: None,
    ///     log: Default::default(),
    ///     specs: vec![Spec { name: "custom-spec".to_string(), ..Default::default() }],
    /// };
    ///
    /// let default_spec: Spec = config.find_default_spec();
    /// assert_eq!(default_spec.name, "custom-spec");
    ///
    /// config.default_spec = "non-existent-spec".to_string();
    /// let fallback_spec = config.find_default_spec();
    /// assert_eq!(fallback_spec, Spec::default()); // Falls back to default if not found
    /// ```
    pub fn find_default_spec(&self) -> Spec {
        self.specs.iter().find(|img| img.name == self.default_spec).cloned().unwrap_or_default()
    }

    /// Finds a `Spec` by its name within the `specs` list.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the `Spec` to search for.
    ///
    /// # Returns
    ///
    /// An `Option` containing a cloned `Spec` if a match is found, otherwise
    /// `None`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use axon::config::{Config, Spec};
    ///
    /// let config = Config {
    ///     default_pod_name: "my-pod".to_string(),
    ///     default_spec: "my-spec".to_string(),
    ///     ssh_private_key_file_path: None,
    ///     log: Default::default(),
    ///     specs: vec![
    ///         Spec { name: "my-spec".to_string(), ..Default::default() },
    ///         Spec { name: "another-spec".to_string(), ..Default::default() },
    ///     ],
    /// };
    ///
    /// let found_spec: Option<Spec> = config.find_spec_by_name("my-spec");
    /// assert!(found_spec.is_some());
    /// assert_eq!(found_spec.unwrap().name, "my-spec");
    ///
    /// let not_found_spec: Option<Spec> = config.find_spec_by_name("non-existent");
    /// assert!(not_found_spec.is_none());
    /// ```
    pub fn find_spec_by_name(&self, name: &str) -> Option<Spec> {
        self.specs.iter().find(|img| img.name == name).cloned()
    }

    /// Provides a basic YAML template for the application's configuration.
    ///
    /// This template can be used as a starting point for creating a new
    /// configuration file.
    ///
    /// # Returns
    ///
    /// A `Vec<u8>` containing the bytes of the `basic.yaml` template.
    pub fn template_basic() -> Vec<u8> { include_bytes!("templates/basic.yaml").to_vec() }
}

/// Returns the default pod name.
///
/// This function is used as a default value provider for the `default_pod_name`
/// field in the `Config` struct.
///
/// # Returns
///
/// A `String` containing the default pod name.
fn default_pod_name() -> String { DEFAULT_POD_NAME.to_string() }

/// Returns the default project name, which serves as the default spec name.
///
/// This function is used as a default value provider for the `default_spec`
/// field in the `Config` struct.
///
/// # Returns
///
/// A `String` containing the default spec name, typically the project name.
fn default_spec() -> String { PROJECT_NAME.to_string() }

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn test_templates() {
        let _basic = serde_yaml::from_slice::<Config>(&Config::template_basic()).unwrap();
    }
}
