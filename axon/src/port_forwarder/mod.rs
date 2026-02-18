#![allow(dead_code)]

mod error;

use std::{
    future::Future,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

use futures::{
    FutureExt, StreamExt,
    future::{self, Either},
};
use k8s_openapi::api::core::v1::Pod;
use kube::Api;
use sigfinn::{ExitStatus, Handle};
use tokio::net::TcpListener;

pub use self::error::Error;

pub struct PortForwarder<F>
where
    F: FnOnce(SocketAddr) + Send + 'static,
{
    api: Api<Pod>,
    pod_name: String,
    local_addr: SocketAddr,
    remote_port: u16,
    handle: Handle<Error>,
    on_ready: Option<F>,
}

pub struct PortForwarderBuilder<F> {
    api: Api<Pod>,
    pod_name: String,
    local_addr: Option<SocketAddr>,
    remote_port: u16,
    handle: Handle<Error>,
    on_ready: Option<F>,
}

impl<F> PortForwarderBuilder<F>
where
    F: FnOnce(SocketAddr) + Send + 'static,
{
    pub fn new(
        api: Api<Pod>,
        pod_name: impl Into<String>,
        remote_port: u16,
        handle: Handle<Error>,
    ) -> Self {
        Self {
            api,
            pod_name: pod_name.into(),
            remote_port,
            handle,
            local_addr: None,
            on_ready: None,
        }
    }

    pub fn local_address(mut self, addr: SocketAddr) -> Self {
        self.local_addr = Some(addr);
        self
    }

    pub fn on_ready(self, callback: F) -> PortForwarderBuilder<F> {
        PortForwarderBuilder {
            api: self.api,
            pod_name: self.pod_name,
            local_addr: self.local_addr,
            remote_port: self.remote_port,
            handle: self.handle,
            on_ready: Some(callback),
        }
    }

    pub fn build(self) -> PortForwarder<F> {
        let local_addr =
            self.local_addr.unwrap_or_else(|| SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0));

        PortForwarder {
            api: self.api,
            pod_name: self.pod_name,
            local_addr,
            remote_port: self.remote_port,
            handle: self.handle,
            on_ready: self.on_ready,
        }
    }
}

impl<F> PortForwarder<F>
where
    F: FnOnce(SocketAddr) + Send + 'static,
{
    pub async fn run(self, shutdown_signal: impl Future<Output = ()> + Unpin) -> ExitStatus<Error> {
        let Self { api, pod_name, local_addr, remote_port, handle, on_ready } = self;

        // 1. Bind the local TCP Listener
        let listener = match TcpListener::bind(&local_addr).await {
            Ok(l) => l,
            Err(source) => {
                return ExitStatus::Error(Error::BindTcpSocket {
                    socket_address: local_addr,
                    source,
                });
            }
        };

        let actual_addr = match listener.local_addr() {
            Ok(addr) => addr,
            Err(source) => {
                return ExitStatus::Error(Error::BindTcpSocket {
                    socket_address: local_addr,
                    source,
                });
            }
        };

        tracing::info!("Forwarding from: {actual_addr} -> {pod_name}:{remote_port}");

        // 2. Trigger the readiness callback
        if let Some(on_ready) = on_ready {
            on_ready(actual_addr);
        }

        let mut shutdown_stream = shutdown_signal.into_stream();

        // 3. Main Accept Loop
        loop {
            let maybe_connection = tokio::select! {
                _ = shutdown_stream.next() => break,
                connection = listener.accept() => connection,
            };

            match maybe_connection {
                Err(source) => {
                    return ExitStatus::Error(Error::AcceptTcpSocket {
                        socket_address: actual_addr,
                        source,
                    });
                }
                Ok((mut local_stream, peer)) => {
                    let api = api.clone();
                    let pod_name = pod_name.clone();
                    let stream_id = format!("stream-{actual_addr}-{}", peer.port());

                    // 4. Spawn the bidirectional bridge for each connection
                    let _unused =
                        handle.spawn(stream_id.clone(), move |conn_shutdown| async move {
                            let pf_res = api
                                .portforward(&pod_name, &[remote_port])
                                .await
                                .map(|mut pf| pf.take_stream(remote_port));

                            let mut pod_stream = match pf_res {
                                Ok(Some(stream)) => stream,
                                Ok(None) => return ExitStatus::Success,
                                Err(source) => {
                                    tracing::error!(
                                        "Failed to initialize pod stream for {stream_id}"
                                    );
                                    return ExitStatus::Error(Error::CreatePodStream {
                                        stream_id,
                                        source: Box::new(source),
                                    });
                                }
                            };

                            tracing::info!("Creating connection [{actual_addr}->{remote_port}]");
                            let copy_fut =
                                tokio::io::copy_bidirectional(&mut local_stream, &mut pod_stream);
                            tokio::pin!(copy_fut);

                            match future::select(conn_shutdown, copy_fut).await {
                                Either::Left(_) => tracing::info!("Closing stream due to shutdown"),
                                Either::Right((Err(e), _)) => tracing::warn!("Stream error: {e}"),
                                Either::Right((Ok(_), _)) => {}
                            }

                            ExitStatus::Success
                        });
                }
            }
        }

        ExitStatus::Success
    }
}
