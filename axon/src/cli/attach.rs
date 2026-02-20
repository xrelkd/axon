//! Defines the `attach` subcommand for connecting to an interactive shell in a
//! Kubernetes pod.
//!
//! This module provides the `AttachCommand` struct and its implementation,
//! enabling users to attach to a running pod and execute an interactive shell.
//! It handles resolving pod identity, waiting for pod readiness, and delegating
//! the shell session management to `PodConsole`.

use std::time::Duration;

use clap::Args;
use k8s_openapi::api::core::v1::Pod;
use kube::Api;

use crate::{
    cli::{
        Error,
        internal::{ApiPodExt, ResolvedResources, ResourceResolver},
    },
    config::Config,
    ext::PodExt,
    pod_console::PodConsole,
};

/// Represents the command to attach to an interactive shell within a Kubernetes
/// pod.
///
/// This struct defines the arguments available for the `attach` subcommand,
/// allowing users to specify the target namespace, pod name, desired
/// interactive shell, and a timeout.
#[derive(Args, Clone)]
pub struct AttachCommand {
    /// Kubernetes namespace of the target pod.
    ///
    /// If not specified, the default namespace will be used.
    #[arg(
        short,
        long,
        help = "Kubernetes namespace of the target pod. If not specified, the default namespace \
                will be used."
    )]
    pub namespace: Option<String>,

    /// Name of the temporary pod to attach to.
    ///
    /// If not specified, Axon's default pod name will be used.
    #[arg(
        short = 'p',
        long = "pod-name",
        help = "Name of the temporary pod to attach to. If not specified, Axon's default pod name \
                will be used."
    )]
    pub pod_name: Option<String>,

    /// Command and arguments for the interactive shell to use.
    ///
    /// For example: `/bin/bash` or `bash -c 'sh'`. If not specified, Axon will
    /// attempt to detect the shell automatically based on the pod's image.
    #[arg(
        short = 's',
        long = "shell",
        help = "Command and arguments for the interactive shell to use (e.g., `/bin/bash`, `bash \
                -c 'sh'`). If not specified, Axon will attempt to detect the shell."
    )]
    pub interactive_shell: Vec<String>,

    /// The maximum time in seconds to wait for the pod to be running before
    /// timing out.
    ///
    /// Defaults to 15 seconds.
    #[arg(
        short = 't',
        long = "timeout-seconds",
        default_value = "15",
        help = "The maximum time in seconds to wait for the pod to be running before timing out."
    )]
    pub timeout_secs: u64,
}

impl AttachCommand {
    /// Executes the `attach` command, connecting to an interactive shell in a
    /// specified Kubernetes pod.
    ///
    /// This asynchronous function resolves the target pod's identity, waits for
    /// the pod to reach a running state, determines the interactive shell
    /// to use, and then delegates the actual shell session management to
    /// `PodConsole`.
    ///
    /// # Arguments
    ///
    /// * `self` - The `AttachCommand` instance containing the parsed
    ///   command-line arguments.
    /// * `kube_client` - A Kubernetes client used to interact with the API
    ///   server.
    /// * `config` - The application's configuration, used for resolving
    ///   resources.
    ///
    /// # Errors
    ///
    /// This function returns an `Error` if:
    ///
    /// * The pod cannot be resolved or accessed via the Kubernetes API.
    /// * The pod does not reach a running state within the configured
    ///   `timeout_secs`.
    /// * An error occurs during the establishment or operation of the
    ///   interactive console session.
    pub async fn run(self, kube_client: kube::Client, config: Config) -> Result<(), Error> {
        let Self { namespace, pod_name, interactive_shell, timeout_secs } = self;

        // Resolve Identity
        let ResolvedResources { namespace, pod_name } =
            ResourceResolver::from((&kube_client, &config)).resolve(namespace, pod_name);

        // Resolve Pod API & Status
        let api = Api::<Pod>::namespaced(kube_client, &namespace);
        let pod = api
            .await_running_status(&pod_name, &namespace, Duration::from_secs(timeout_secs))
            .await?;

        // Resolve Shell
        let shell =
            if interactive_shell.is_empty() { pod.interactive_shell() } else { interactive_shell };

        // Delegate behavior
        PodConsole::new(api, pod_name, namespace, shell).run().await.map_err(Error::from)
    }
}
