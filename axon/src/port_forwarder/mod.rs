//! A module for managing Kubernetes port-forwarding sessions.
//!
//! This module provides the `PortForwarder` struct, which can be used to
//! establish and maintain a TCP port-forwarding connection from a local address
//! to a specific port on a Kubernetes Pod. It handles connection setup,
//! lifecycle management, and graceful shutdown.
//!
//! # Example
//! ```no_run
//! use std::{net::{SocketAddr, IpAddr, Ipv4Addr}, time::Duration};
//! use axon::port_forwarder::{PortForwarderBuilder, Error};
//! use kube::Client;
//! use k8s_openapi::api::core::v1::Pod;
//! use kube::Api;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Error> {
//!     // In a real application, you'd get a proper Kube client
//!     let client = Client::try_default().await.expect("Failed to create kube client");
//!     let api: Api<Pod> = Api::namespaced(client, "default");
//!
//!     let pod_name = "my-app-pod".to_string();
//!     let remote_port = 8080;
//!     let local_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8000);
//!
//!     let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel(1);
//!
//!     let forwarder = PortForwarderBuilder::new(api, pod_name, remote_port)
//!         .local_address(local_addr)
//!         .on_ready(|addr| {
//!             println!("Port forwarding ready at {}", addr);
//!             // You can send a signal here to notify the application that
//!             // port forwarding is established.
//!         })
//!         .build();
//!
//!     // Simulate a shutdown signal after some time
//!     tokio::spawn(async move {
//!         tokio::time::sleep(Duration::from_secs(60)).await;
//!         let _ = shutdown_tx.send(()).await;
//!     });
//!
//!     forwarder.run(async {
//!         let _ = shutdown_rx.recv().await;
//!         println!("Shutdown signal received, stopping port forwarder.");
//!     }).await?;
//!
//!     println!("Port forwarder stopped.");
//!     Ok(())
//! }
//! ```
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

/// Internal events that drive the `PortForwarder`'s main loop.
enum Event {
    /// Signals the port forwarder to shut down gracefully.
    Shutdown,
    /// Indicates a new incoming TCP connection from a local client.
    NewConnection {
        /// The new local TCP stream.
        stream: TcpStream,
        /// The address of the peer that initiated the connection.
        peer: SocketAddr,
    },
    /// Signals the port forwarder to clean up any completed or failed
    /// connections.
    ReapConnections,
}

/// Manages a Kubernetes port-forwarding session, bridging local TCP connections
/// to a specified port on a remote Pod.
pub struct PortForwarder<F>
where
    F: FnOnce(SocketAddr) + Send + 'static,
{
    /// Kubernetes API client for interacting with Pods.
    api: Api<Pod>,
    /// The name of the Pod to which connections will be forwarded.
    pod_name: String,
    /// The local address that the forwarder will bind to and listen on.
    local_addr: SocketAddr,
    /// The target port on the remote Pod.
    remote_port: u16,
    /// An optional callback function executed once the local listener is ready.
    /// It receives the actual local address the forwarder is listening on.
    on_ready: Option<F>,
    /// A set of spawned Tokio tasks managing individual connections and
    /// internal operations.
    join_set: JoinSet<Result<(), Error>>,
}

/// A builder for creating a `PortForwarder` instance.
///
/// This builder allows for configuring the local address, remote port, and an
/// optional `on_ready` callback before constructing the `PortForwarder`.
pub struct PortForwarderBuilder<F> {
    /// Kubernetes API client for interacting with Pods.
    api: Api<Pod>,
    /// The name of the Pod to which connections will be forwarded.
    pod_name: String,
    /// The optional local address for the forwarder to bind to. If `None`, a
    /// default (localhost, ephemeral port) will be used.
    local_addr: Option<SocketAddr>,
    /// The target port on the remote Pod.
    remote_port: u16,
    /// An optional callback function to be executed once the local listener is
    /// ready.
    on_ready: Option<F>,
}

impl<F> PortForwarderBuilder<F> {
    /// Creates a new `PortForwarderBuilder`.
    ///
    /// # Arguments
    ///
    /// * `api` - A Kubernetes API client configured for Pod resources.
    /// * `pod_name` - The name of the target Pod.
    /// * `remote_port` - The port on the target Pod to forward to.
    ///
    /// # Returns
    ///
    /// A new `PortForwarderBuilder` instance.
    ///
    /// # Example
    /// ```no_run
    /// use axon_port_forwarder::PortForwarderBuilder;
    /// use kube::Client;
    /// use k8s_openapi::api::core::v1::Pod;
    /// use kube::Api;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let client = Client::try_default().await.unwrap();
    ///     let api: Api<Pod> = Api::namespaced(client, "default");
    ///     let builder = PortForwarderBuilder::new(api, "my-pod", 8080);
    /// }
    /// ```
    pub fn new(api: Api<Pod>, pod_name: impl Into<String>, remote_port: u16) -> Self {
        Self { api, pod_name: pod_name.into(), remote_port, local_addr: None, on_ready: None }
    }

    /// Sets the local address for the port forwarder to bind to.
    ///
    /// If not set, the forwarder will bind to `127.0.0.1:0` (localhost on an
    /// ephemeral port).
    ///
    /// # Arguments
    ///
    /// * `addr` - The `SocketAddr` for the local listener.
    ///
    /// # Returns
    ///
    /// The modified `PortForwarderBuilder` instance.
    ///
    /// # Example
    /// ```no_run
    /// use axon_port_forwarder::PortForwarderBuilder;
    /// use std::net::{SocketAddr, IpAddr, Ipv4Addr};
    /// use kube::Client;
    /// use k8s_openapi::api::core::v1::Pod;
    /// use kube::Api;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let client = Client::try_default().await.unwrap();
    ///     let api: Api<Pod> = Api::namespaced(client, "default");
    ///     let local_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8000);
    ///     let builder = PortForwarderBuilder::new(api, "my-pod", 8080)
    ///         .local_address(local_addr);
    /// }
    /// ```
    pub const fn local_address(mut self, addr: SocketAddr) -> Self {
        self.local_addr = Some(addr);
        self
    }
}

impl<F> PortForwarderBuilder<F>
where
    F: FnOnce(SocketAddr) + Send + 'static,
{
    /// Sets a callback function to be executed once the port forwarder's local
    /// listener is successfully bound and ready to accept connections.
    ///
    /// The callback receives the actual `SocketAddr` the forwarder is listening
    /// on.
    ///
    /// # Arguments
    ///
    /// * `callback` - A closure that takes a `SocketAddr` and returns `()`.
    ///
    /// # Returns
    ///
    /// The modified `PortForwarderBuilder` instance.
    ///
    /// # Example
    /// ```no_run
    /// use axon::port_forwarder::PortForwarderBuilder;
    /// use kube::Client;
    /// use k8s_openapi::api::core::v1::Pod;
    /// use kube::Api;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let client = Client::try_default().await.unwrap();
    ///     let api: Api<Pod> = Api::namespaced(client, "default");
    ///     let builder = PortForwarderBuilder::new(api, "my-pod", 8080)
    ///         .on_ready(|addr| {
    ///             println!("Forwarding available on: {}", addr);
    ///         });
    /// }
    /// ```
    pub fn on_ready(self, callback: F) -> Self {
        Self {
            api: self.api,
            pod_name: self.pod_name,
            local_addr: self.local_addr,
            remote_port: self.remote_port,
            on_ready: Some(callback),
        }
    }

    /// Builds the `PortForwarder` instance from the configured builder.
    ///
    /// If no local address was specified, it defaults to `127.0.0.1:0`.
    ///
    /// # Returns
    ///
    /// A new `PortForwarder` instance.
    ///
    /// # Example
    /// ```no_run
    /// use axon::port_forwarder::PortForwarderBuilder;
    /// use kube::Client;
    /// use k8s_openapi::api::core::v1::Pod;
    /// use kube::Api;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let client = Client::try_default().await.unwrap();
    ///     let api: Api<Pod> = Api::namespaced(client, "default");
    ///     let forwarder = PortForwarderBuilder::new(api, "my-pod", 8080)
    ///         .build();
    /// }
    /// ```
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
    /// Starts the port-forwarding process and runs until a shutdown signal is
    /// received or an unrecoverable error occurs.
    ///
    /// This method sets up a local TCP listener, accepts incoming connections,
    /// and bridges them to the specified remote port on the Kubernetes Pod.
    /// It gracefully handles shutdown signals and cleans up active
    /// connections.
    ///
    /// # Arguments
    ///
    /// * `shutdown_signal` - An asynchronous future that completes when a
    ///   shutdown should be initiated.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success (`Ok(())`) or an `Error` if the forwarder
    /// encounters a problem.
    ///
    /// # Errors
    ///
    /// This function can return an `Error` in the following cases:
    ///
    /// * `Error::BindTcpSocket { socket_address }`: If the local TCP listener
    ///   cannot bind to the specified `local_addr` or determine its
    ///   `local_addr`.
    /// * Any errors originating from the `kube` client during port-forwarding
    ///   setup or connection handling are propagated as `Error::KubeError`.
    /// * Any `io::Error` during bidirectional copying of data between streams
    ///   are wrapped as `Error::IoError`.
    ///
    /// # Example
    /// ```no_run
    /// use std::{net::{SocketAddr, IpAddr, Ipv4Addr}, time::Duration};
    /// use axon::port_forwarder::{PortForwarderBuilder, Error};
    /// use kube::Client;
    /// use k8s_openapi::api::core::v1::Pod;
    /// use kube::Api;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Error> {
    ///     let client = Client::try_default().await.expect("Failed to create kube client");
    ///     let api: Api<Pod> = Api::namespaced(client, "default");
    ///
    ///     let pod_name = "my-app-pod".to_string();
    ///     let remote_port = 8080;
    ///     let local_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8000);
    ///
    ///     let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel(1);
    ///
    ///     let forwarder = PortForwarderBuilder::new(api, pod_name, remote_port)
    ///         .local_address(local_addr)
    ///         .on_ready(|addr| {
    ///             println!("Port forwarding ready at {}", addr);
    ///         })
    ///         .build();
    ///
    ///     // In a real application, this signal might come from a Ctrl+C handler or similar.
    ///     tokio::spawn(async move {
    ///         tokio::time::sleep(Duration::from_secs(5)).await;
    ///         let _ = shutdown_tx.send(()).await;
    ///     });
    ///
    ///     forwarder.run(async {
    ///         let _ = shutdown_rx.recv().await;
    ///     }).await?;
    ///
    ///     println!("Port forwarder gracefully shut down.");
    ///     Ok(())
    /// }
    /// ```
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

/// Encapsulates the configuration and logic needed to bridge a single local TCP
/// connection to a Kubernetes Pod's port-forwarding stream.
#[derive(Clone)]
struct ConnectionHandler {
    /// Kubernetes API client for interacting with Pods.
    api: Api<Pod>,
    /// The name of the Pod to which the connection will be forwarded.
    pod_name: String,
    /// The target port on the remote Pod.
    remote_port: u16,
    /// The actual local address the `PortForwarder` is listening on.
    actual_addr: SocketAddr,
    /// A cancellation token to signal immediate shutdown to active connections.
    cancel_token: CancellationToken,
}

impl ConnectionHandler {
    /// Creates a new `ConnectionHandler` instance by cloning the current one.
    ///
    /// This is used to create a distinct handler for each new incoming
    /// connection, allowing it to capture the necessary configuration.
    ///
    /// # Returns
    ///
    /// A new `ConnectionHandler` instance.
    ///
    /// # Example
    /// ```
    /// use axon::port_forwarder::Error;
    /// use k8s_openapi::api::core::v1::Pod;
    /// use kube::Api;
    /// use tokio_util::sync::CancellationToken;
    /// use std::net::{SocketAddr, IpAddr, Ipv4Addr};
    ///
    /// // Assume `api`, `pod_name`, `remote_port`, `actual_addr`, `cancel_token` are initialized
    /// # async fn doc_example() -> Result<(), Error> {
    /// # let client = kube::Client::try_default().await.unwrap();
    /// # let api: Api<Pod> = Api::namespaced(client, "default");
    /// # let pod_name = "test-pod".to_string();
    /// # let remote_port = 8080;
    /// # let actual_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8000);
    /// # let cancel_token = CancellationToken::new();
    /// let base_handler = ConnectionHandler {
    ///     api, pod_name, remote_port, actual_addr, cancel_token
    /// };
    /// let new_handler = base_handler.create();
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    fn create(&self) -> Self { self.clone() }

    /// Handles a single incoming local TCP connection, bridging it to a
    /// Kubernetes Pod.
    ///
    /// This asynchronous function establishes a port-forwarding stream to the
    /// target Pod and then copies data bidirectionally between the local
    /// client stream and the Pod's stream. It respects the provided
    /// `cancel_token` for graceful shutdown.
    ///
    /// # Arguments
    ///
    /// * `local_stream` - The incoming local `TcpStream` from the client.
    /// * `peer` - The `SocketAddr` of the connected local peer.
    ///
    /// # Returns
    ///
    /// A `Result` indicating success (`Ok(())`) or an `Error` if the bridging
    /// fails.
    ///
    /// # Errors
    ///
    /// This function can return an `Error` in the following cases:
    ///
    /// * `Error::CreatePodStream { stream_id, source }`: If there is an issue
    ///   establishing the Kubernetes port-forwarding stream to the Pod. The
    ///   `source` will contain the underlying error from the `kube` client.
    /// * Any `io::Error` during bidirectional copying of data between streams
    ///   are wrapped as `Error::IoError`.
    ///
    /// # Example
    /// ```no_run
    /// use axon::port_forwarder::Error;
    /// use k8s_openapi::api::core::v1::Pod;
    /// use kube::Api;
    /// use tokio_util::sync::CancellationToken;
    /// use std::net::{SocketAddr, IpAddr, Ipv4Addr};
    /// use tokio::net::TcpStream;
    ///
    /// // Assume `api`, `pod_name`, `remote_port`, `actual_addr`, `cancel_token` are initialized
    /// // and `local_stream`, `peer` are from an accepted connection.
    /// # async fn doc_example() -> Result<(), Error> {
    /// # let client = kube::Client::try_default().await.unwrap();
    /// # let api: Api<Pod> = Api::namespaced(client, "default");
    /// # let pod_name = "test-pod".to_string();
    /// # let remote_port = 8080;
    /// # let actual_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8000);
    /// # let cancel_token = CancellationToken::new();
    /// # let (mut local_stream, _) = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap().accept().await.unwrap();
    /// # let peer = local_stream.peer_addr().unwrap();
    /// let handler = ConnectionHandler {
    ///     api, pod_name, remote_port, actual_addr, cancel_token
    /// };
    /// handler.handle(local_stream, peer).await?;
    /// # Ok(())
    /// # }
    /// ```
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
            Ok(None) => {
                // Port forward stream not found, connection ignored.
                return Ok(());
            }
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
