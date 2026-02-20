//! This module defines the `ServicePorts` struct, which represents a collection
//! of optional service ports for SSH, HTTP, and HTTPS. It provides
//! functionality to convert between this struct and Kubernetes annotation
//! key-value pairs.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::consts::k8s::annotations;

/// Represents a collection of optional service ports for SSH, HTTP, and HTTPS.
///
/// This struct is used to manage and serialize/deserialize port configurations,
/// particularly in the context of Kubernetes annotations.
#[derive(Clone, Debug, Default, Deserialize, Eq, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServicePorts {
    /// The SSH port, if specified.
    pub ssh: Option<u16>,

    /// The HTTP port, if specified.
    pub http: Option<u16>,

    /// The HTTPS port, if specified.
    pub https: Option<u16>,
}

impl ServicePorts {
    /// Creates a new `ServicePorts` instance with common default ports (SSH:
    /// 22, HTTP: 80, HTTPS: 443).
    ///
    /// # Returns
    ///
    /// A `ServicePorts` instance with `ssh`, `http`, and `https` fields set to
    /// their common defaults.
    #[allow(dead_code)]
    pub const fn common() -> Self { Self { ssh: Some(22), http: Some(80), https: Some(443) } }

    /// Aggregates multiple Kubernetes annotations into a single `ServicePorts`
    /// struct.
    ///
    /// This function iterates over a collection of key-value pairs, parsing
    /// each as a potential service port annotation and merging them into a
    /// single `ServicePorts` instance.
    ///
    /// # Arguments
    ///
    /// * `iter` - An iterator over items that can be converted into displayable
    ///   key-value pairs. Each key and value will be converted to a string to
    ///   check against Kubernetes service port annotation format.
    ///
    /// # Returns
    ///
    /// A `ServicePorts` instance representing the aggregated ports from the
    /// provided annotations.
    pub fn from_kubernetes_annotations<I, K, V>(iter: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: fmt::Display,
        V: fmt::Display,
    {
        iter.into_iter().fold(Self::default(), |mut acc, (k, v)| {
            acc.merge(&Self::from_kubernetes_annotation(k, v));
            acc
        })
    }

    /// Merges another `ServicePorts` struct into this one.
    ///
    /// If a port is `Some` in `other`, it will overwrite the corresponding port
    /// in `self`. If a port is `None` in `other`, the corresponding port in
    /// `self` remains unchanged.
    ///
    /// # Arguments
    ///
    /// * `other` - A reference to another `ServicePorts` instance to merge
    ///   from.
    const fn merge(&mut self, other: &Self) {
        if let Some(p) = other.ssh {
            self.ssh = Some(p);
        }
        if let Some(p) = other.http {
            self.http = Some(p);
        }
        if let Some(p) = other.https {
            self.https = Some(p);
        }
    }

    /// Creates a `ServicePorts` instance from a single Kubernetes annotation
    /// key-value pair.
    ///
    /// This function attempts to parse the provided `key` and `value` to
    /// extract a service port (ssh, http, or https) if it matches the
    /// expected Kubernetes annotation format.
    ///
    /// # Arguments
    ///
    /// * `key` - The key of the Kubernetes annotation. Expected to be in the
    ///   format `annotations::SERVICE_PORT_PREFIX/<port_type>`.
    /// * `value` - The value of the Kubernetes annotation, expected to be a
    ///   string representation of a `u16` port.
    ///
    /// # Returns
    ///
    /// A `ServicePorts` instance with the parsed port set, or
    /// `ServicePorts::default()` if the key does not match the expected
    /// format or the value cannot be parsed as a `u16`.
    pub fn from_kubernetes_annotation<K, V>(key: K, value: V) -> Self
    where
        K: fmt::Display,
        V: fmt::Display,
    {
        let key_str = key.to_string();
        let val_str = value.to_string();
        let prefix = format!("{}/", *annotations::SERVICE_PORT_PREFIX);

        let mut ports = Self::default();

        // Check if the key starts with our expected prefix
        if let Some(suffix) = key_str.strip_prefix(&prefix)
            && let Ok(port) = val_str.parse::<u16>()
        {
            match suffix {
                "ssh" => ports.ssh = Some(port),
                "http" => ports.http = Some(port),
                "https" => ports.https = Some(port),
                _ => {}
            }
        }

        ports
    }

    /// Converts the `ServicePorts` instance into a vector of Kubernetes
    /// annotation key-value pairs.
    ///
    /// Each defined port (ssh, http, https) will be converted into a `(String,
    /// String)` tuple, formatted according to the Kubernetes annotation
    /// convention using `annotations::SERVICE_PORT_PREFIX`.
    ///
    /// # Returns
    ///
    /// A `Vec<(String, String)>` where each tuple represents a Kubernetes
    /// annotation for a service port.
    pub fn to_kubernetes_annotation(&self) -> Vec<(String, String)> {
        let Self { ssh, http, https } = self;
        let mut kv = Vec::with_capacity(3);
        let prefix = annotations::SERVICE_PORT_PREFIX.as_str();
        if let Some(ssh) = ssh {
            kv.push((format!("{prefix}/ssh"), format!("{ssh}")));
        }
        if let Some(http) = http {
            kv.push((format!("{prefix}/http"), format!("{http}")));
        }
        if let Some(https) = https {
            kv.push((format!("{prefix}/https"), format!("{https}")));
        }
        kv
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_annotation_valid() {
        let key = format!("{}/http", *annotations::SERVICE_PORT_PREFIX);
        let val = "8080";
        let ports = ServicePorts::from_kubernetes_annotation(key, val);

        assert_eq!(ports.http, Some(8080));
        assert_eq!(ports.ssh, None);
    }

    #[test]
    fn test_from_annotation_invalid_prefix() {
        let ports = ServicePorts::from_kubernetes_annotation("wrong.io/ssh", "22");
        assert_eq!(ports.ssh, None);
    }

    #[test]
    fn test_from_annotation_invalid_value() {
        let key = format!("{}/https", *annotations::SERVICE_PORT_PREFIX);
        let ports = ServicePorts::from_kubernetes_annotation(key, "not-a-number");
        assert_eq!(ports.https, None);
    }

    #[test]
    fn test_to_annotations_serialization() {
        let ports = ServicePorts { ssh: Some(22), http: Some(80), https: None };

        let result = ports.to_kubernetes_annotation();

        assert_eq!(result.len(), 2);
        assert!(
            result.contains(&(
                format!("{}/ssh", *annotations::SERVICE_PORT_PREFIX),
                "22".to_string()
            ))
        );
        assert!(
            result.contains(&(
                format!("{}/http", *annotations::SERVICE_PORT_PREFIX),
                "80".to_string()
            ))
        );
    }

    #[test]
    fn test_round_trip() {
        // Testing that what we output can be read back in
        let original = ServicePorts { ssh: Some(2222), ..Default::default() };

        let annotations = original.to_kubernetes_annotation();
        let (key, val) = &annotations[0];

        let recovered = ServicePorts::from_kubernetes_annotation(key, val);
        assert_eq!(original, recovered);
    }
}
