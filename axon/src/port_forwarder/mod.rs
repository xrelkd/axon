mod error;

use std::{
    future::Future,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

use k8s_openapi::api::core::v1::Pod;
use kube::Api;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc,
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;

pub use self::error::Error;

/// Internal events that drive the Forwarder loop
enum Event {
    Shutdown,
    NewConnection { stream: TcpStream, peer: SocketAddr },
    ReapConnections,
}

pub struct PortForwarder<F>
where
    F: FnOnce(SocketAddr) + Send + 'static,
{
    api: Api<Pod>,
    pod_name: String,
    local_addr: SocketAddr,
    remote_port: u16,
    join_set: JoinSet<Result<(), Error>>,
    on_ready: Option<F>,
}

pub struct PortForwarderBuilder<F> {
    api: Api<Pod>,
    pod_name: String,
    local_addr: Option<SocketAddr>,
    remote_port: u16,
    on_ready: Option<F>,
}

impl<F> PortForwarderBuilder<F>
where
    F: FnOnce(SocketAddr) + Send + 'static,
{
    pub fn new(api: Api<Pod>, pod_name: impl Into<String>, remote_port: u16) -> Self {
        Self { api, pod_name: pod_name.into(), remote_port, local_addr: None, on_ready: None }
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
            join_set: JoinSet::new(),
            on_ready: self.on_ready,
        }
    }
}

impl<F> PortForwarder<F>
where
    F: FnOnce(SocketAddr) + Send + 'static,
{
    pub async fn run(
        self,
        shutdown_signal: impl Future<Output = ()> + Send + Unpin + 'static,
    ) -> Result<(), Error> {
        let Self { api, pod_name, local_addr, remote_port, mut join_set, on_ready } = self;

        let listener = TcpListener::bind(&local_addr)
            .await
            .map_err(|source| Error::BindTcpSocket { socket_address: local_addr, source })?;

        let actual_addr = listener
            .local_addr()
            .map_err(|source| Error::BindTcpSocket { socket_address: local_addr, source })?;

        tracing::info!("Forwarding from: {actual_addr} -> {pod_name}:{remote_port}");

        if let Some(on_ready) = on_ready {
            on_ready(actual_addr);
        }

        // --- Orchestration Tools ---
        let (event_sender, mut event_receiver) = mpsc::channel::<Event>(32);
        let cancel_token = CancellationToken::new();

        // 1. Shutdown Watcher Task
        // Listens for the external signal and triggers the internal cancellation
        let tx_shutdown = event_sender.clone();
        let token_shutdown = cancel_token.clone();
        join_set.spawn(async move {
            tokio::select! {
                _ = shutdown_signal => {
                    tracing::debug!("Shutdown signal received, notifying loop...");
                    let _ = tx_shutdown.send(Event::Shutdown).await;
                }
                _ = token_shutdown.cancelled() => {
                    tracing::debug!("Shutdown task cancelled internally.");
                    let _ = tx_shutdown.send(Event::Shutdown).await;
                }
            }
            Ok(())
        });

        // 2. Accept Task
        let tx_accept = event_sender.clone();
        let token_accept = cancel_token.clone();
        join_set.spawn(async move {
            loop {
                let maybe_conn = tokio::select! {
                    _ = token_accept.cancelled() => break,
                    conn = listener.accept() => conn,
                };

                if let Ok((stream, peer)) = maybe_conn
                    && tx_accept.send(Event::NewConnection { stream, peer }).await.is_err()
                {
                    break;
                }
            }
            tracing::debug!("Accept task exited.");
            Ok(())
        });

        // 3. Reap/Timer Task
        let tx_reap = event_sender.clone();
        let token_reap = cancel_token.clone();
        join_set.spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                tokio::select! {
                    _ = token_reap.cancelled() => break,
                    _ = interval.tick() => {}
                }
                if tx_reap.send(Event::ReapConnections).await.is_err() {
                    break;
                }
            }
            tracing::debug!("Reap task exited.");
            Ok(())
        });

        while let Some(event) = event_receiver.recv().await {
            match event {
                Event::Shutdown => {
                    tracing::info!("Initiating graceful shutdown...");
                    cancel_token.cancel(); // Signal all background tasks to stop
                    break;
                }
                Event::ReapConnections => {
                    while let Some(result) = join_set.try_join_next() {
                        if let Ok(Err(e)) = result {
                            tracing::error!("Connection error during reap: {e}");
                        }
                    }
                }
                Event::NewConnection { mut stream, peer } => {
                    let api = api.clone();
                    let pod_name = pod_name.clone();
                    let stream_id = format!("stream-{actual_addr}-{}", peer.port());
                    let token_conn = cancel_token.clone();

                    join_set.spawn(async move {
                        let pf_res = api
                            .portforward(&pod_name, &[remote_port])
                            .await
                            .map(|mut pf| pf.take_stream(remote_port));

                        let mut pod_stream = match pf_res {
                            Ok(Some(s)) => s,
                            Ok(None) => return Ok(()),
                            Err(source) => {
                                return Err(Error::CreatePodStream {
                                    stream_id,
                                    source: Box::new(source),
                                });
                            }
                        };

                        tracing::info!("Bridging connection for peer {peer}");

                        // We use select here so individual connections also respect the global
                        // shutdown
                        tokio::select! {
                            _ = token_conn.cancelled() => {
                                tracing::debug!("Closing connection {} due to shutdown", peer);
                            }
                            _ = tokio::io::copy_bidirectional(&mut stream, &mut pod_stream) => {}
                        }
                        Ok(())
                    });
                }
            }
        }

        // --- Cleanup Phase ---
        // Explicitly drop the receiver so tasks sending events fail (if any are still
        // alive)
        drop(event_receiver);

        tracing::info!("Waiting for all active connections to close...");
        // This will wait for all tasks in the JoinSet to complete
        while let Some(result) = join_set.join_next().await {
            if let Ok(Err(e)) = result {
                tracing::error!("Final cleanup connection error: {e}");
            }
        }

        tracing::info!("Port forwarder exit complete.");
        Ok(())
    }
}
