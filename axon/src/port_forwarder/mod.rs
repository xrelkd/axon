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
use tokio::{net::TcpListener, task::JoinSet};

pub use self::error::Error;

pub struct PortForwarder<F>
where
    F: FnOnce(SocketAddr) + Send + 'static,
{
    api: Api<Pod>,
    pod_name: String,
    local_addr: SocketAddr,
    remote_port: u16,
    handle: JoinSet<Result<(), Error>>,
    on_ready: Option<F>,
}

pub struct PortForwarderBuilder<F> {
    api: Api<Pod>,
    pod_name: String,
    local_addr: Option<SocketAddr>,
    remote_port: u16,
    handle: JoinSet<Result<(), Error>>,
    on_ready: Option<F>,
}

impl<F> PortForwarderBuilder<F>
where
    F: FnOnce(SocketAddr) + Send + 'static,
{
    pub fn new(api: Api<Pod>, pod_name: impl Into<String>, remote_port: u16) -> Self {
        Self {
            api,
            pod_name: pod_name.into(),
            remote_port,
            handle: JoinSet::new(),
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
    pub async fn run(self, shutdown_signal: impl Future<Output = ()> + Unpin) -> Result<(), Error> {
        let Self { api, pod_name, local_addr, remote_port, mut handle, on_ready } = self;

        // 1. Bind the local TCP Listener
        let listener = TcpListener::bind(&local_addr)
            .await
            .map_err(|source| Error::BindTcpSocket { socket_address: local_addr, source })?;

        let actual_addr = listener
            .local_addr()
            .map_err(|source| Error::BindTcpSocket { socket_address: local_addr, source })?;

        tracing::info!("Forwarding from: {actual_addr} -> {pod_name}:{remote_port}");

        // 2. Trigger the readiness callback
        if let Some(on_ready) = on_ready {
            on_ready(actual_addr);
        }

        let mut shutdown_stream = shutdown_signal.into_stream();

        // 3. Main Accept Loop
        loop {
            tokio::select! {
                // Handle global shutdown
                _ = shutdown_stream.next() => {
                    tracing::info!("Shutdown signal received, closing port forwarder");
                    break;
                }

                // Accept new connections
                connection = listener.accept() => {
                    let (mut local_stream, peer) = connection.map_err(|source| Error::AcceptTcpSocket {
                        socket_address: actual_addr,
                        source,
                    })?;

                    let api = api.clone();
                    let pod_name = pod_name.clone();
                    let stream_id = format!("stream-{actual_addr}-{}", peer.port());

                    // 4. Spawn the bidirectional bridge for each connection into the JoinSet
                    handle.spawn(async move {
                        let pf_res = api
                            .portforward(&pod_name, &[remote_port])
                            .await
                            .map(|mut pf| pf.take_stream(remote_port));

                        let mut pod_stream = match pf_res {
                            Ok(Some(stream)) => stream,
                            Ok(None) => return Ok(()),
                            Err(source) => {
                                tracing::error!("Failed to initialize pod stream for {stream_id}");
                                return Err(Error::CreatePodStream {
                                    stream_id,
                                    source: Box::new(source),
                                });
                            }
                        };

                        tracing::info!("Creating connection [{actual_addr}->{remote_port}]");

                        // Copy data until finished or error
                        match tokio::io::copy_bidirectional(&mut local_stream, &mut pod_stream).await {
                            Ok((sent, received)) => {
                                tracing::debug!("Connection closed: sent {sent}, received {received}");
                                Ok(())
                            }
                            Err(e) => {
                                tracing::warn!("Stream error: {e}");
                                Ok(()) // We don't necessarily want to kill the whole forwarder on one stream error
                            }
                        }
                    });
                }

                // Clean up finished tasks from the JoinSet to prevent memory leaks
                Some(result) = handle.join_next() => {
                    if let Ok(Err(e)) = result {
                        tracing::error!("Connection task failed: {e}");
                    }
                }
            }
        }

        // Optional: Wait for remaining connections to finish or abort them
        handle.shutdown().await;
        Ok(())
    }
}
