use std::fmt;

use axon_base::consts::k8s::annotations;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Eq, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServicePorts {
    pub ssh: Option<u16>,

    pub http: Option<u16>,

    pub https: Option<u16>,
}

impl ServicePorts {
    #[allow(dead_code)]
    pub const fn common() -> Self { Self { ssh: Some(22), http: Some(80), https: Some(443) } }

    /// Aggregates multiple annotations into a single ServicePorts struct
    /// from any iterator of key-value pairs.
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

    /// Helper to merge another `ServicePorts` struct into this one,
    /// overwriting existing values if the other has Some.
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
