//! This module provides internal utilities for managing SSH connections within
//! the CLI, including port forwarding setup and file transfer mechanisms.

pub mod configurator;
pub mod file_transfer;
pub mod handle_guard;

use std::net::SocketAddr;

use k8s_openapi::api::core::v1::Pod;
use kube::Api;
use sigfinn::ExitStatus;
use tokio::sync::oneshot;

pub use self::{
    configurator::Configurator,
    file_transfer::{FileTransfer, FileTransferRunner},
    handle_guard::HandleGuard,
};
use crate::{cli::Error, port_forwarder::PortForwarderBuilder};

/// The default SSH port.
pub const DEFAULT_SSH_PORT: u16 = 22;

/// Sets up port forwarding to a specified remote port on a Kubernetes pod.
///
/// This function initializes a port forwarder that listens on a local address
/// and forwards traffic to the `remote_port` of the target `pod_name`.
/// It returns a `oneshot::Receiver` that will yield the local `SocketAddr`
/// once the port forwarding is successfully established.
///
/// # Arguments
///
/// * `api` - The Kubernetes API client for interacting with Pods.
/// * `pod_name` - The name of the target pod for port forwarding. This can be
///   anything that can be converted into a `String` (e.g., `&str`, `String`).
/// * `remote_port` - The port on the target pod to which traffic will be
///   forwarded.
/// * `handle` - A `sigfinn::Handle` used to spawn the port forwarding task,
///   allowing it to be gracefully shut down.
///
/// # Returns
///
/// A `tokio::sync::oneshot::Receiver<SocketAddr>` which will receive the
/// local `SocketAddr` once the port forwarding connection is successfully
/// established and ready to accept connections.
///
/// # Errors
///
/// The spawned port forwarding task can encounter errors during its operation,
/// such as issues connecting to the Kubernetes API, finding the pod, or
/// establishing the port forwarding tunnel. These errors are reported via
/// the `ExitStatus::Error` variant of the `sigfinn` task. The specific
/// error type returned is `crate::cli::Error`.
///
/// # Examples
///
/// ```no_run
/// use std::net::SocketAddr;
/// use std::sync::Arc;
/// use k8s_openapi::api::core::v1::Pod;
/// use kube::{Api, Client};
/// use sigfinn::Handle;
/// use tokio::sync::oneshot;
/// use crate::cli::ssh::internal::setup_port_forwarding; // Assuming this is the correct path
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let client = Client::try_default().await?;
///     let api: Api<Pod> = Api::namespaced(client, "default");
///     let pod_name = "my-ssh-pod".to_string();
///     let remote_port = 22;
///     let sigfinn_handle = sigfinn::Handle::new();
///
///     let receiver = setup_port_forwarding(api, pod_name, remote_port, &sigfinn_handle);
///
///     // Wait for the port forwarder to be ready and get the local address
///     let local_addr = receiver.await?;
///     println!("SSH port forwarding established on: {}", local_addr);
///
///     // In a real application, you would now connect to `local_addr` for SSH.
///     // For demonstration, we'll just simulate some work and then shut down.
///     tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
///
///     println!("Shutting down port forwarder...");
///     sigfinn_handle.shutdown();
///     sigfinn_handle.join().await;
///
///     Ok(())
/// }
/// ```
pub fn setup_port_forwarding(
    api: Api<Pod>,
    pod_name: impl Into<String>,
    remote_port: u16,
    handle: &sigfinn::Handle<Error>,
) -> oneshot::Receiver<SocketAddr> {
    let (sender, receiver) = oneshot::channel();
    let on_ready = move |socket_addr| {
        let _unused = sender.send(socket_addr);
    };
    let pod_name = pod_name.into();
    let _handle = handle.spawn("port-forwarder", move |shutdown_signal| async move {
        let result = PortForwarderBuilder::new(api, pod_name, remote_port)
            .on_ready(on_ready)
            .build()
            .run(shutdown_signal)
            .await;
        match result {
            Ok(()) => ExitStatus::Success,
            Err(err) => ExitStatus::Error(Error::from(err)),
        }
    });
    receiver
}
