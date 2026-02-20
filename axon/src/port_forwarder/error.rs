//! Defines the error types for the port forwarder module.

use std::net::SocketAddr;

use snafu::Snafu;

/// Represents the possible errors that can occur within the port forwarder.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    /// Occurs when the system fails to bind to a specified TCP socket address.
    ///
    /// This error indicates a problem during the initial setup phase of a port
    /// forward, typically due to the address being already in use,
    /// permissions issues, or an invalid address format.
    #[snafu(display("Failed to bind TCP socket {socket_address}, error: {source}"))]
    BindTcpSocket {
        /// The socket address that the system attempted to bind to.
        socket_address: SocketAddr,
        /// The underlying I/O error that occurred.
        source: std::io::Error,
    },

    /// Occurs when there is a failure to create a pod stream.
    ///
    /// This error typically arises when interacting with the Kubernetes API
    /// to establish a connection to a pod, for example, if the pod does not
    /// exist, or if there are network or authentication issues.
    #[snafu(display("Failed to create pod stream {stream_id}, error: {source}"))]
    CreatePodStream {
        /// The identifier of the stream that failed to be created.
        stream_id: String,
        /// The underlying error from the `kube` client library.
        #[snafu(source(from(kube::Error, Box::new)))]
        source: Box<kube::Error>,
    },
}
