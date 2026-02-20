//! Defines the `execute` command for running arbitrary commands within a
//! Kubernetes pod.

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
    pod_console::PodConsole,
};

/// Represents the `execute` command and its arguments.
///
/// This command allows users to run arbitrary shell commands inside a specified
/// Kubernetes pod.
#[derive(Args, Clone)]
pub struct ExecuteCommand {
    /// Kubernetes namespace of the target pod.
    ///
    /// If not specified, Axon will attempt to determine the default namespace.
    #[arg(
        short,
        long,
        help = "Kubernetes namespace of the target pod. If not specified, the default namespace \
                will be used."
    )]
    pub namespace: Option<String>,

    /// Name of the temporary pod to execute the command on.
    ///
    /// If not specified, Axon's default pod naming convention will be used.
    #[arg(
        short = 'p',
        long = "pod-name",
        help = "Name of the temporary pod to execute the command on. If not specified, Axon's \
                default pod name will be used."
    )]
    pub pod_name: Option<String>,

    /// The maximum time in seconds to wait for the pod to be running before
    /// timing out.
    #[arg(
        short = 't',
        long = "timeout-seconds",
        default_value = "15",
        help = "The maximum time in seconds to wait for the pod to be running before timing out."
    )]
    pub timeout_secs: u64,

    /// The command and its arguments to execute inside the container.
    ///
    /// This argument is required and should be provided as a list of strings,
    /// where the first string is the command itself and subsequent strings are
    /// its arguments.
    #[arg(
        help = "The command and its arguments to execute inside the container.",
        required = true
    )]
    pub command: Vec<String>,
}

impl ExecuteCommand {
    /// Executes the specified command within a Kubernetes pod.
    ///
    /// This asynchronous function resolves the target pod's namespace and name,
    /// waits for the pod to be in a running state, and then initiates a console
    /// session to run the provided command.
    ///
    /// # Arguments
    ///
    /// * `self` - The `ExecuteCommand` instance containing the command details.
    /// * `kube_client` - A `kube::Client` instance for interacting with the
    ///   Kubernetes API.
    /// * `config` - The application's `Config` settings.
    ///
    /// # Errors
    ///
    /// This function returns an `Err` variant of `Error` if:
    ///
    /// * The target namespace or pod name cannot be resolved.
    /// * The specified pod does not reach a running state within the
    ///   `timeout_secs`.
    /// * There's an issue connecting to the pod's console or executing the
    ///   command.
    ///
    /// # Panics
    ///
    /// This method does not explicitly panic, but underlying `kube` or `tokio`
    /// operations could potentially panic in extreme error scenarios (e.g.,
    /// OOM).
    pub async fn run(self, kube_client: kube::Client, config: Config) -> Result<(), Error> {
        let Self { namespace, pod_name, command, timeout_secs } = self;

        // Resolve Identity
        let ResolvedResources { namespace, pod_name } =
            ResourceResolver::from((&kube_client, &config)).resolve(namespace, pod_name);

        // Resolve Pod API & Status
        let api = Api::<Pod>::namespaced(kube_client, &namespace);
        let _pod = api
            .await_running_status(&pod_name, &namespace, Duration::from_secs(timeout_secs))
            .await?;

        PodConsole::new(api, pod_name, namespace, command).run().await.map_err(Error::from)
    }
}
