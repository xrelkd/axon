use std::net::SocketAddr;

use snafu::Snafu;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    #[snafu(display("Failed to bind TCP socket {socket_address}, error: {source}"))]
    BindTcpSocket { socket_address: SocketAddr, source: std::io::Error },

    #[snafu(display("Failed to create pod stream {stream_id}, error: {source}"))]
    CreatePodStream {
        stream_id: String,
        #[snafu(source(from(kube::Error, Box::new)))]
        source: Box<kube::Error>,
    },
}
