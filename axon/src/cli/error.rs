//! Defines the `Error` enum, which represents all possible errors that can
//! occur within the `cli` module.
//!
//! This module centralizes error handling, providing a consistent way to manage
//! and propagate various error conditions encountered during CLI operations,
//! such as configuration issues, Kubernetes API failures, SSH problems, and UI
//! interaction errors.

use snafu::Snafu;

/// Represents all possible errors that can occur within the `cli` module.
///
/// This enum consolidates error types from various sub-modules and external
/// dependencies into a single, manageable error type, allowing for consistent
/// error handling and reporting across the CLI application.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    /// A generic error with a customizable message.
    #[snafu(display("{message}"))]
    Generic {
        /// The detailed error message.
        message: String,
    },

    /// An error originating from the configuration module.
    #[snafu(display("{source}"))]
    Configuration { source: crate::config::Error },

    /// An error originating from the SSH module.
    #[snafu(display("{source}"))]
    Ssh { source: crate::ssh::Error },

    /// An error originating from the terminal UI module.
    #[snafu(display("{source}"))]
    TerminalUi { source: crate::ui::terminal::Error },

    /// An error originating from the port forwarder module.
    #[snafu(display("{source}"))]
    PortForwarder { source: crate::port_forwarder::Error },

    /// An error originating from the pod console module.
    #[snafu(display("{source}"))]
    PodConsole { source: crate::pod_console::Error },

    /// An error indicating that a specified image specification was not found.
    #[snafu(display("Image specification '{spec_name}' not found"))]
    SpecNotFound {
        /// The name of the image specification that was not found.
        spec_name: String,
    },

    /// An error that occurs when failing to write to stdout.
    #[snafu(display("Failed to write to stdout, error: {source}"))]
    WriteStdout { source: std::io::Error },

    /// An error indicating a failure to initialize the Kubernetes client
    /// configuration.
    #[snafu(display("Failed to initialize Kubernetes client configuration, error: {source}"))]
    KubeConfig { source: kube::Error },

    /// An error that occurs when failing to create a Kubernetes pod.
    #[snafu(display("Failed to create pod {pod_name} in namespace {namespace}, error: {source}"))]
    CreatePod {
        /// The namespace where the pod creation failed.
        namespace: String,
        /// The name of the pod that failed to be created.
        pod_name: String,

        #[snafu(source(from(kube::Error, Box::new)))]
        source: Box<kube::Error>,
    },

    /// An error that occurs when failing to delete a Kubernetes pod.
    #[snafu(display("Failed to delete pod {pod_name} in namespace {namespace}, error: {source}"))]
    DeletePod {
        /// The namespace where the pod deletion failed.
        namespace: String,
        /// The name of the pod that failed to be deleted.
        pod_name: String,

        #[snafu(source(from(kube::Error, Box::new)))]
        source: Box<kube::Error>,
    },

    /// An error that occurs when failing to list Kubernetes pods.
    #[snafu(display("Failed to list pods, error: {source}"))]
    ListPods {
        #[snafu(source(from(kube::Error, Box::new)))]
        source: Box<kube::Error>,
    },

    /// An error that occurs when failing to get the status of a specific
    /// Kubernetes pod.
    #[snafu(display(
        "Failed to get pod {pod_name} status in namespace {namespace}, error: {source}"
    ))]
    GetPod {
        /// The namespace of the pod.
        namespace: String,
        /// The name of the pod.
        pod_name: String,
        /// The underlying `kube::Error`.
        #[snafu(source(from(kube::Error, Box::new)))]
        source: Box<kube::Error>,
    },

    /// An error indicating a timeout occurred while waiting for a pod to reach
    /// a running status.
    #[snafu(display(
        "Timed out waiting for pod '{pod_name}' to reach running status in namespace '{namespace}'"
    ))]
    WaitForPodStatus {
        /// The namespace of the pod.
        namespace: String,
        /// The name of the pod.
        pod_name: String,
    },

    /// An error that occurs when failing to wait for a Kubernetes pod's status.
    #[snafu(display(
        "Failed to wait for pod {pod_name} status in namespace {namespace}, error: {source}"
    ))]
    GetPodStatus {
        /// The namespace of the pod.
        namespace: String,
        /// The name of the pod.
        pod_name: String,

        #[snafu(source(from(kube::runtime::wait::Error, Box::new)))]
        source: Box<kube::runtime::wait::Error>,
    },

    /// An error that occurs when failing to list pods within a specific
    /// namespace.
    #[snafu(display("Failed to list pods in namespace {namespace}, error: {source}"))]
    ListPodsWithNamespace {
        /// The namespace where the pod listing failed.
        namespace: String,

        #[snafu(source(from(kube::Error, Box::new)))]
        source: Box<kube::Error>,
    },

    /// An error that occurs when failing to initialize a Tokio runtime.
    #[snafu(display("Failed to create tokio runtime, error: {source}"))]
    InitializeTokioRuntime { source: std::io::Error },

    /// An error that occurs when failing to upload or authorize an SSH key in a
    /// pod.
    #[snafu(display("Failed to upload or authorize SSH key in pod '{pod_name}', error: {source}"))]
    UploadSshKey {
        /// The namespace of the pod.
        namespace: String,
        /// The name of the pod.
        pod_name: String,

        #[snafu(source(from(kube::Error, Box::new)))]
        source: Box<kube::Error>,
    },

    /// An error that occurs when failing to serialize interactive shell
    /// configuration.
    #[snafu(display("Failed to serialize interactive shell configuration, error: {source}"))]
    SerializeInteractiveShell { source: serde_json::Error },
}

/// Implements conversion from `crate::config::Error` to `Error::Configuration`.
impl From<crate::config::Error> for Error {
    /// Converts a `crate::config::Error` into an `Error::Configuration`
    /// variant.
    ///
    /// # Arguments
    ///
    /// * `source` - The `crate::config::Error` to convert.
    ///
    /// # Returns
    ///
    /// An `Error::Configuration` containing the original error.
    fn from(source: crate::config::Error) -> Self { Self::Configuration { source } }
}

/// Implements conversion from `crate::ssh::Error` to `Error::Ssh`.
impl From<crate::ssh::Error> for Error {
    /// Converts a `crate::ssh::Error` into an `Error::Ssh` variant.
    ///
    /// # Arguments
    ///
    /// * `source` - The `crate::ssh::Error` to convert.
    ///
    /// # Returns
    ///
    /// An `Error::Ssh` containing the original error.
    fn from(source: crate::ssh::Error) -> Self { Self::Ssh { source } }
}

/// Implements conversion from `crate::ui::terminal::Error` to
/// `Error::TerminalUi`.
impl From<crate::ui::terminal::Error> for Error {
    /// Converts a `crate::ui::terminal::Error` into an `Error::TerminalUi`
    /// variant.
    ///
    /// # Arguments
    ///
    /// * `source` - The `crate::ui::terminal::Error` to convert.
    ///
    /// # Returns
    ///
    /// An `Error::TerminalUi` containing the original error.
    fn from(source: crate::ui::terminal::Error) -> Self { Self::TerminalUi { source } }
}

/// Implements conversion from `crate::port_forwarder::Error` to
/// `Error::PortForwarder`.
impl From<crate::port_forwarder::Error> for Error {
    /// Converts a `crate::port_forwarder::Error` into an `Error::PortForwarder`
    /// variant.
    ///
    /// # Arguments
    ///
    /// * `source` - The `crate::port_forwarder::Error` to convert.
    ///
    /// # Returns
    ///
    /// An `Error::PortForwarder` containing the original error.
    fn from(source: crate::port_forwarder::Error) -> Self { Self::PortForwarder { source } }
}

/// Implements conversion from `crate::pod_console::Error` to
/// `Error::PodConsole`.
impl From<crate::pod_console::Error> for Error {
    /// Converts a `crate::pod_console::Error` into an `Error::PodConsole`
    /// variant.
    ///
    /// # Arguments
    ///
    /// * `source` - The `crate::pod_console::Error` to convert.
    ///
    /// # Returns
    ///
    /// An `Error::PodConsole` containing the original error.
    fn from(source: crate::pod_console::Error) -> Self { Self::PodConsole { source } }
}
