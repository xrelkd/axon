use std::{
    fmt,
    net::{IpAddr, SocketAddr},
    str::FromStr,
};

use axon_base::consts::k8s::annotations;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortMapping {
    pub container_port: u16,

    pub local_port: u16,

    pub address: IpAddr,
}

impl PortMapping {
    pub fn to_kubernetes_annotation(&self) -> (String, String) {
        let Self { container_port, local_port, address } = self;
        (
            format!("{}/{container_port}", *annotations::PORT_MAPPINGS_PREFIX),
            format!("{address}:{local_port}",),
        )
    }

    /// Parses a `PortMapping` from a key (containing the container port)
    /// and a value (containing the address and local port).
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

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Snafu, PartialEq, Eq)]
#[snafu(visibility(pub))]
pub enum PortMappingError {
    #[snafu(display(
        "Invalid format: expected 'ADDRESS:LOCAL_PORT:CONTAINER_PORT', got '{input}'",
    ))]
    InvalidFormat { input: String },

    #[snafu(display("Invalid port value '{value}': {source}"))]
    InvalidPort { value: String, source: std::num::ParseIntError },

    #[snafu(display("Invalid IP address '{value}': {source}"))]
    InvalidAddress { value: String, source: std::net::AddrParseError },
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
