//! Defines the `PortMapping` struct and related error types for handling port
//! configurations.
//!
//! This module provides the `PortMapping` struct, which represents a mapping
//! between a container port, a local port, and an IP address. It includes
//! functionality for converting `PortMapping` instances to and from Kubernetes
//! annotation strings, as well as parsing from a string representation.

use std::{
    fmt,
    net::{IpAddr, SocketAddr},
    str::FromStr,
};

use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};

use crate::consts::k8s::annotations;

/// Represents a mapping between a container port, a local port, and an IP
/// address.
///
/// This struct is used to define how a port inside a container is exposed on
/// the host machine, allowing for flexible network configurations.
///
/// # Examples
/// ```
/// use std::net::IpAddr;
/// use axon::config::PortMapping;
///
/// let mapping = PortMapping {
///     container_port: 80,
///     local_port: 8080,
///     address: "127.0.0.1".parse().unwrap(),
/// };
///
/// assert_eq!(mapping.container_port, 80);
/// assert_eq!(mapping.local_port, 8080);
/// assert_eq!(mapping.address, "127.0.0.1".parse::<IpAddr>().unwrap());
/// ```
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortMapping {
    /// The port number inside the container.
    pub container_port: u16,

    /// The port number on the local host machine.
    pub local_port: u16,

    /// The IP address on which the `local_port` is exposed.
    pub address: IpAddr,
}

impl PortMapping {
    /// Converts the `PortMapping` into a key-value pair suitable for Kubernetes
    /// annotations.
    ///
    /// The key is formatted as `PORT_MAPPINGS_PREFIX/container_port`, and the
    /// value is formatted as `address:local_port`.
    ///
    /// # Returns
    /// A tuple `(String, String)` representing the annotation key and value.
    pub fn to_kubernetes_annotation(&self) -> (String, String) {
        let Self { container_port, local_port, address } = self;
        (
            format!("{}/{container_port}", *annotations::PORT_MAPPINGS_PREFIX),
            format!("{address}:{local_port}"),
        )
    }

    /// Parses a `PortMapping` from a Kubernetes annotation key and value.
    ///
    /// The key is expected to be in the format `prefix/container_port`,
    /// and the value in the format `address:local_port`.
    ///
    /// # Type Parameters
    /// - `K`: Type that can be displayed as a string, representing the
    ///   annotation key.
    /// - `V`: Type that can be displayed as a string, representing the
    ///   annotation value.
    ///
    /// # Arguments
    /// * `key` - The annotation key string, e.g., "axon.dev/port-mapping/80".
    /// * `value` - The annotation value string, e.g., "127.0.0.1:8080".
    ///
    /// # Errors
    /// Returns a `PortMappingError` if:
    /// - The `key` does not contain a `/` to separate the prefix from the
    ///   container port.
    /// - The extracted container port from the `key` is not a valid `u16`.
    /// - The `value` cannot be parsed into a valid `SocketAddr` (e.g.,
    ///   malformed IP address or port).
    ///
    /// # Examples
    /// ```
    /// use std::net::IpAddr;
    /// use axon::config::port_mapping::{PortMapping, PortMappingError};
    /// use axon::consts::k8s::annotations;
    ///
    /// let key = format!("{}/8080", *annotations::PORT_MAPPINGS_PREFIX);
    /// let value = "127.0.0.1:80";
    ///
    /// let mapping = PortMapping::try_from_kubernetes_annotation(key, value)
    ///     .expect("Valid annotation should parse");
    ///
    /// assert_eq!(mapping.container_port, 8080);
    /// assert_eq!(mapping.local_port, 80);
    /// assert_eq!(mapping.address, "127.0.0.1".parse::<IpAddr>().unwrap());
    ///
    /// // Example of an invalid value
    /// let invalid_value = "not.an.ip.address:80";
    /// let error = PortMapping::try_from_kubernetes_annotation(key, invalid_value).unwrap_err();
    /// assert!(matches!(error, PortMappingError::InvalidFormat { .. }));
    /// ```
    pub fn try_from_kubernetes_annotation<K, V>(key: K, value: V) -> Result<Self, PortMappingError>
    where
        K: fmt::Display,
        V: fmt::Display,
    {
        let key = key.to_string();
        let value = value.to_string();

        // Extract container_port from key: "prefix/container_port"
        let container_port_str = key
            .split('/')
            .next_back()
            .ok_or_else(|| PortMappingError::InvalidFormat { input: key.clone() })?;

        let container_port = container_port_str
            .parse::<u16>()
            .context(InvalidPortSnafu { value: container_port_str.to_string() })?;

        // Parse Address and Local Port using SocketAddr
        // SocketAddr handles both "127.0.0.1:80" and "[::1]:80" automatically
        let socket_addr = value.parse::<SocketAddr>().map_err(|_| {
            // Note: If parsing fails, it's usually because the address
            // format is wrong or the port is missing/invalid.
            PortMappingError::InvalidFormat { input: value.clone() }
        })?;

        Ok(Self { container_port, local_port: socket_addr.port(), address: socket_addr.ip() })
    }
}

impl FromStr for PortMapping {
    type Err = PortMappingError;

    #[allow(clippy::doc_markdown)]
    /// Parses a `PortMapping` from a string in the format
    /// `ADDRESS:LOCAL_PORT:CONTAINER_PORT`.
    ///
    /// This implementation is designed to correctly handle both IPv4 and IPv6
    /// addresses by splitting the string from the right.
    ///
    /// # Arguments
    /// * `input` - The string slice to parse, e.g., "127.0.0.1:7070:8080" or
    ///   "::1:7070:8080".
    ///
    /// # Errors
    /// Returns a `PortMappingError` if:
    /// - The `input` string does not contain exactly two colon separators.
    /// - The `container_port` or `local_port` parts are not valid `u16`
    ///   integers.
    /// - The `address` part is not a valid `IpAddr`.
    ///
    /// # Examples
    /// ```
    /// use std::net::IpAddr;
    /// use std::str::FromStr;
    /// use axon_config::config::port_mapping::{PortMapping, PortMappingError};
    ///
    /// // IPv4 example
    /// let mapping_v4 = PortMapping::from_str("127.0.0.1:7070:8080")
    ///     .expect("Should parse valid IPv4 mapping");
    /// assert_eq!(mapping_v4.address, "127.0.0.1".parse::<IpAddr>().unwrap());
    /// assert_eq!(mapping_v4.local_port, 7070);
    /// assert_eq!(mapping_v4.container_port, 8080);
    ///
    /// // IPv6 example (handles colons in IPv6 address correctly)
    /// let mapping_v6 = PortMapping::from_str("::1:7070:8080")
    ///     .expect("Should parse valid IPv6 mapping");
    /// assert_eq!(mapping_v6.address, "::1".parse::<IpAddr>().unwrap());
    /// assert_eq!(mapping_v6.local_port, 7070);
    /// assert_eq!(mapping_v6.container_port, 8080);
    ///
    /// // Error example
    /// let error = PortMapping::from_str("127.0.0.1:8080").unwrap_err();
    /// assert!(matches!(error, PortMappingError::InvalidFormat { .. }));
    /// ```
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        // Use rsplitn(3, ':') to handle IPv6 addresses correctly.
        // It ensures we extract the two ports from the right first.
        let parts: Vec<&str> = input.rsplitn(3, ':').collect();

        if parts.len() != 3 {
            return InvalidFormatSnafu { input }.fail();
        }

        // parts[0] is container_port, parts[1] is local_port, parts[2] is address
        let container_port =
            parts[0].parse::<u16>().context(InvalidPortSnafu { value: parts[0] })?;

        let local_port = parts[1].parse::<u16>().context(InvalidPortSnafu { value: parts[1] })?;

        let address =
            parts[2].parse::<IpAddr>().context(InvalidAddressSnafu { value: parts[2] })?;

        Ok(Self { container_port, local_port, address })
    }
}

/// Represents possible errors that can occur when parsing or creating a
/// `PortMapping`.
#[allow(clippy::enum_variant_names)]
#[derive(Debug, Snafu, PartialEq, Eq)]
#[snafu(visibility(pub))]
pub enum PortMappingError {
    /// Indicates that the input string for a `PortMapping` had an invalid
    /// format.
    ///
    /// Expected format: `ADDRESS:LOCAL_PORT:CONTAINER_PORT`.
    #[snafu(display(
        "Invalid format: expected 'ADDRESS:LOCAL_PORT:CONTAINER_PORT', got '{input}'",
    ))]
    InvalidFormat {
        /// The input string that caused the error.
        input: String,
    },

    /// Indicates that a port value could not be parsed as a valid `u16`.
    #[snafu(display("Invalid port value '{value}', error: {source}"))]
    InvalidPort {
        /// The invalid string value that was attempted to be parsed as a port.
        value: String,
        /// The underlying parsing error.
        source: std::num::ParseIntError,
    },

    /// Indicates that an IP address string could not be parsed as a valid
    /// `IpAddr`.
    #[snafu(display("Invalid IP address '{value}', error: {source}"))]
    InvalidAddress {
        /// The invalid string value that was attempted to be parsed as an IP
        /// address.
        value: String,
        /// The underlying parsing error.
        source: std::net::AddrParseError,
    },
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    #[test]
    fn test_parse_ipv4_mapping() {
        let input = "127.0.0.1:7070:8080";
        let result: PortMapping = input.parse().expect("Should parse valid IPv4");

        assert_eq!(result.address, "127.0.0.1".parse::<IpAddr>().unwrap());
        assert_eq!(result.local_port, 7070);
        assert_eq!(result.container_port, 8080);
    }

    #[test]
    fn test_parse_ipv6_mapping() {
        // rsplitn correctly treats "::1" as the address even with internal colons
        let input = "::1:7070:8080";
        let result: PortMapping = input.parse().expect("Should parse valid IPv6");

        assert_eq!(result.address, "::1".parse::<IpAddr>().unwrap());
        assert_eq!(result.local_port, 7070);
        assert_eq!(result.container_port, 8080);
    }

    #[test]
    fn test_error_invalid_format() {
        let input = "127.0.0.1:8080";
        let err = input.parse::<PortMapping>().unwrap_err();
        assert!(matches!(err, PortMappingError::InvalidFormat { .. }));
    }

    #[test]
    fn test_error_invalid_port() {
        let input = "127.0.0.1:7070:not_a_number";
        let err = input.parse::<PortMapping>().unwrap_err();
        assert!(matches!(err, PortMappingError::InvalidPort { .. }));
    }

    #[test]
    fn test_error_invalid_ip() {
        let input = "localhost:7070:8080"; // IpAddr doesn't resolve hostnames
        let err = input.parse::<PortMapping>().unwrap_err();
        assert!(matches!(err, PortMappingError::InvalidAddress { .. }));
    }

    #[test]
    fn test_parse_valid_mapping() {
        let key = format!("{}/8080", *annotations::PORT_MAPPINGS_PREFIX);
        let value = "127.0.0.1:80";
        let result = PortMapping::try_from_kubernetes_annotation(key, value).unwrap();

        assert_eq!(result.container_port, 8080);
        assert_eq!(result.local_port, 80);
        assert_eq!(result.address, IpAddr::V4(Ipv4Addr::LOCALHOST));
    }

    #[test]
    fn test_invalid_port_error() {
        let key = format!("{}/not_a_port", *annotations::PORT_MAPPINGS_PREFIX);
        let value = "127.0.0.1:80";
        let result = PortMapping::try_from_kubernetes_annotation(key, value);

        assert!(matches!(result, Err(PortMappingError::InvalidPort { .. })));
    }

    #[test]
    fn test_invalid_address_error() {
        let key = format!("{}/8080", *annotations::PORT_MAPPINGS_PREFIX);
        let value = "not.an.ip.address:80";
        let result = PortMapping::try_from_kubernetes_annotation(key, value);

        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_format_error() {
        let key = format!("{}/8080", *annotations::PORT_MAPPINGS_PREFIX);
        let value = "127.0.0.1-80"; // Wrong separator
        let result = PortMapping::try_from_kubernetes_annotation(key, value);

        assert!(matches!(result, Err(PortMappingError::InvalidFormat { .. })));
    }

    #[test]
    fn test_try_from_ipv6() {
        let key = format!("{}/443", *annotations::PORT_MAPPINGS_PREFIX);

        // SocketAddr requires brackets for IPv6 to disambiguate colons
        let value = "[2001:db8::1]:8443";

        let result =
            PortMapping::try_from_kubernetes_annotation(key, value).expect("Should parse IPv6");

        assert_eq!(result.address, "2001:db8::1".parse::<IpAddr>().unwrap());
        assert_eq!(result.local_port, 8443);
        assert_eq!(result.container_port, 443);
    }

    #[test]
    fn test_invalid_socket_format() {
        let key = format!("{}/80", *annotations::PORT_MAPPINGS_PREFIX);

        // Missing brackets for IPv6 or missing port will fail SocketAddr parsing
        let value = "2001:db8::1:80";
        let result = PortMapping::try_from_kubernetes_annotation(key, value);

        assert!(result.is_err());
    }
}
