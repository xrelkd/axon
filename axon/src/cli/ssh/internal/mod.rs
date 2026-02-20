mod configurator;
mod file_transfer;
mod handle_guard;

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

pub const DEFAULT_SSH_PORT: u16 = 22;

pub fn setup_port_forwarding(
    api: Api<Pod>,
    pod_name: impl Into<String> + Send + 'static,
    remote_port: u16,
    handle: &sigfinn::Handle<Error>,
) -> oneshot::Receiver<SocketAddr> {
    let (sender, receiver) = oneshot::channel();
    let on_ready = move |socket_addr| {
        let _unused = sender.send(socket_addr);
    };
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
