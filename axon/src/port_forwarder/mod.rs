mod error;
use std::{
    future::Future,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

use k8s_openapi::api::core::v1::Pod;
use kube::Api;
use snafu::{IntoError, ResultExt};
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
    on_ready: Option<F>,
    join_set: JoinSet<Result<(), Error>>,
}

pub struct PortForwarderBuilder<F> {
    api: Api<Pod>,
    pod_name: String,
    local_addr: Option<SocketAddr>,
    remote_port: u16,
    on_ready: Option<F>,
}

impl<F> PortForwarderBuilder<F> {
    pub fn new(api: Api<Pod>, pod_name: impl Into<String>, remote_port: u16) -> Self {
        Self { api, pod_name: pod_name.into(), remote_port, local_addr: None, on_ready: None }
    }

    pub const fn local_address(mut self, addr: SocketAddr) -> Self {
        self.local_addr = Some(addr);
        self
    }
}

impl<F> PortForwarderBuilder<F>
where
    F: FnOnce(SocketAddr) + Send + 'static,
{
    pub fn on_ready(self, callback: F) -> Self {
        Self {
            api: self.api,
            pod_name: self.pod_name,
            local_addr: self.local_addr,
            remote_port: self.remote_port,
            on_ready: Some(callback),
        }
    }

    pub fn build(self) -> PortForwarder<F> {
        let Self { api, pod_name, local_addr, remote_port, on_ready } = self;
        let local_addr =
            local_addr.unwrap_or_else(|| SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0));
        PortForwarder { api, pod_name, local_addr, remote_port, on_ready, join_set: JoinSet::new() }
    }
}

impl<F> PortForwarder<F>
where
    F: FnOnce(SocketAddr) + Send + 'static,
{
    #[allow(clippy::too_many_lines)]
    pub async fn run(
        self,
        shutdown_signal: impl Future<Output = ()> + Send + Unpin + 'static,
    ) -> Result<(), Error> {
        let Self { api, pod_name, local_addr, remote_port, on_ready, mut join_set } = self;

        let listener = TcpListener::bind(&local_addr)
            .await
            .with_context(|_| error::BindTcpSocketSnafu { socket_address: local_addr })?;

        let actual_addr = listener
            .local_addr()
            .with_context(|_| error::BindTcpSocketSnafu { socket_address: local_addr })?;

        tracing::info!("Forwarding from: {actual_addr} -> {pod_name}:{remote_port}");

        if let Some(on_ready) = on_ready {
            on_ready(actual_addr);
        }

        // Orchestration Tools
        let (event_sender, mut event_receiver) = mpsc::unbounded_channel();
        let cancel_token = CancellationToken::new();

        // 1. Shutdown Watcher Task
        // Listens for the external signal and triggers the internal cancellation
        let _unused = join_set.spawn({
            let event_sender = event_sender.clone();
            let token_shutdown = cancel_token.clone();
            async move {
                tokio::select! {
                    () = shutdown_signal => {
                        tracing::debug!("Shutdown signal received, notifying loop...");
                        drop(event_sender.send(Event::Shutdown));
                    }
                    () = token_shutdown.cancelled() => {
                        tracing::debug!("Shutdown task cancelled internally.");
                        drop(event_sender.send(Event::Shutdown));
                    }
                }
                Ok(())
            }
        });

        // 2. Accept Task
        let _unused = join_set.spawn({
            let event_sender = event_sender.clone();
            let token_accept = cancel_token.clone();

            async move {
                loop {
                    let conn = tokio::select! {
                        () = token_accept.cancelled() => break,
                        conn = listener.accept() => conn,
                    };

                    if let Ok((stream, peer)) = conn
                        && event_sender.send(Event::NewConnection { stream, peer }).is_err()
                    {
                        break;
                    }
                }
                tracing::debug!("Accept task exited.");
                Ok(())
            }
        });

        // 3. Reap/Timer Task
        let _unused = join_set.spawn({
            let event_sender = event_sender.clone();
            let token_reap = cancel_token.clone();
            async move {
                let mut interval = tokio::time::interval(Duration::from_secs(5));
                loop {
                    tokio::select! {
                        () = token_reap.cancelled() => break,
                        _ = interval.tick() => {}
                    }
                    if event_sender.send(Event::ReapConnections).is_err() {
                        break;
                    }
                }
                tracing::debug!("Reap task exited.");
                Ok(())
            }
        });

        // Create the base handler template
        let connection_handler_factory = ConnectionHandler {
            api,
            pod_name,
            remote_port,
            actual_addr,
            cancel_token: cancel_token.clone(),
        };

        while let Some(event) = event_receiver.recv().await {
            match event {
                Event::Shutdown => {
                    tracing::info!("Initiating graceful shutdown...");
                    // Signal all background tasks to stop
                    cancel_token.cancel();
                    break;
                }
                Event::ReapConnections => {
                    while let Some(result) = join_set.try_join_next() {
                        if let Ok(Err(e)) = result {
                            tracing::error!("Connection error during reap: {e}");
                        }
                    }
                }
                Event::NewConnection { stream, peer } => {
                    let _unused =
                        join_set.spawn(connection_handler_factory.create().handle(stream, peer));
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

/// Encapsulates the configuration needed to bridge a local connection to a K8s
/// Pod.
#[derive(Clone)]
struct ConnectionHandler {
    api: Api<Pod>,
    pod_name: String,
    remote_port: u16,
    actual_addr: SocketAddr,
    cancel_token: CancellationToken,
}

impl ConnectionHandler {
    #[inline]
    fn create(&self) -> Self { self.clone() }

    async fn handle(self, mut local_stream: TcpStream, peer: SocketAddr) -> Result<(), Error> {
        let Self { api, pod_name, remote_port, actual_addr, cancel_token } = self;

        let stream_id = format!("stream-{actual_addr}-{}", peer.port());

        // Establish the Kubernetes Portforward stream
        let pf_res = api
            .portforward(&pod_name, &[remote_port])
            .await
            .map(|mut pf| pf.take_stream(remote_port));

        let mut pod_stream = match pf_res {
            Ok(Some(s)) => s,
            Ok(None) => return Ok(()),
            Err(source) => return Err(error::CreatePodStreamSnafu { stream_id }.into_error(source)),
        };

        tracing::info!("Bridging connection: {peer} <-> {pod_name}:{remote_port}");

        tokio::select! {
            () = cancel_token.cancelled() => {
                tracing::debug!("Closing connection {peer} due to shutdown");
            }
            res = tokio::io::copy_bidirectional(&mut local_stream, &mut pod_stream) => {
                if let Err(err) = res {
                    tracing::debug!("Connection {peer} closed with error: {err}");
                }
            }
        }
        Ok(())
    }
}
