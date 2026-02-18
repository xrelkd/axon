use std::{net::SocketAddr, time::Duration};

use axon_base::{consts, consts::k8s::annotations};
use futures::{
    FutureExt, StreamExt,
    future::{self, Either},
};
use k8s_openapi::api::core::v1::Pod;
use kube::{
    Api,
    runtime::{conditions::is_pod_running, wait::await_condition},
};
use sigfinn::ExitStatus;
use snafu::ResultExt;
use tokio::net::TcpListener;

use crate::{error, error::Error};

pub trait ApiPodExt {
    async fn interactive_shell(&self, pod_name: &str) -> Vec<String>;

    async fn await_running_status(
        &self,
        pod_name: &str,
        namespace: &str,
        timeout: Duration,
    ) -> Result<Option<Pod>, Error>;

    async fn port_forward<R>(
        &self,
        pod_name: &str,
        local_socket_address: SocketAddr,
        remote_port: u16,
        handle: sigfinn::Handle<Error>,
        shutdown_signal: impl Future<Output = ()> + Unpin,
        on_ready: R,
    ) -> ExitStatus<Error>
    where
        R: FnOnce(SocketAddr) -> ();
}

impl ApiPodExt for Api<Pod> {
    async fn interactive_shell(&self, pod_name: &str) -> Vec<String> {
        if let Ok(pod) = self.get(pod_name).await
            && let Some(annotations) = pod.metadata.annotations
            && let Some(shell_json) = annotations.get(annotations::SHELL_INTERACTIVE.as_str())
            && let Ok(shell) = serde_json::from_str::<Vec<String>>(shell_json)
            && !shell.is_empty()
        {
            shell
        } else {
            consts::DEFAULT_INTERACTIVE_SHELL.iter().map(ToString::to_string).collect()
        }
    }

    async fn await_running_status(
        &self,
        pod_name: &str,
        namespace: &str,
        timeout: Duration,
    ) -> Result<Option<Pod>, Error> {
        // Wait until the pod is running, otherwise we get 500 error.
        tokio::time::timeout(timeout, await_condition(self.clone(), pod_name, is_pod_running()))
            .await
            .map_err(|_| Error::WaitForPodStatus {
                namespace: namespace.to_string(),
                pod_name: pod_name.to_string(),
            })?
            .with_context(|_| error::GetPodStatusSnafu {
                namespace: namespace.to_string(),
                pod_name: pod_name.to_string(),
            })
    }

    async fn port_forward<R>(
        &self,
        pod_name: &str,
        local_socket_address: SocketAddr,
        remote_port: u16,
        handle: sigfinn::Handle<Error>,
        shutdown_signal: impl Future<Output = ()> + Unpin,
        on_ready: R,
    ) -> ExitStatus<Error>
    where
        R: FnOnce(SocketAddr) -> (),
    {
        let listener = match TcpListener::bind(&local_socket_address).await {
            Ok(listener) => listener,
            Err(source) => {
                return ExitStatus::Error(Error::BindTcpSocket {
                    socket_address: local_socket_address,
                    source,
                });
            }
        };
        let local_socket_address = match listener.local_addr() {
            Ok(addr) => addr,
            Err(source) => {
                return ExitStatus::Error(Error::BindTcpSocket {
                    socket_address: local_socket_address,
                    source,
                });
            }
        };

        tracing::info!("Forwarding from: {local_socket_address} -> {pod_name}:{remote_port}");
        on_ready(local_socket_address);

        let mut shutdown_signal = shutdown_signal.into_stream();
        loop {
            let maybe_connection = tokio::select! {
                _ = shutdown_signal.next() => break,
                connection = listener.accept() => connection,
            };

            match maybe_connection {
                Err(source) => {
                    return ExitStatus::Error(Error::AcceptTcpSocket {
                        socket_address: local_socket_address,
                        source,
                    });
                }
                Ok((mut local_stream, peer)) => {
                    let api = self.clone();
                    let pod_name = pod_name.to_string();
                    let stream_id = format!("stream-{local_socket_address}-{}", peer.port());

                    // Use the handle to manage the bidirectional copy as a tracked task
                    let _handle =
                        handle.spawn(stream_id.clone(), move |conn_shutdown| async move {
                            let pf_res = api
                                .portforward(&pod_name, &[remote_port])
                                .await
                                .map(|mut pf| pf.take_stream(remote_port));

                            let mut pod_stream = match pf_res {
                                Ok(Some(stream)) => stream,
                                Ok(None) => return ExitStatus::Success,
                                Err(source) => {
                                    tracing::error!("Failed to initialize pod stream");
                                    return ExitStatus::Error(Error::CreatePodStream {
                                        stream_id: stream_id.clone(),
                                        source: Box::new(source),
                                    });
                                }
                            };

                            // bridge the streams
                            tracing::info!(
                                "Creating connection [{local_socket_address}->{remote_port}]"
                            );
                            let copy_fut =
                                tokio::io::copy_bidirectional(&mut local_stream, &mut pod_stream);
                            tokio::pin!(copy_fut);

                            // If the app shuts down, we stop copying
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
