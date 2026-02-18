use std::net::SocketAddr;

use snafu::Snafu;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    #[snafu(display("{source}"))]
    Ssh { source: crate::ssh::Error },

    #[snafu(display("{source}"))]
    TerminalUi { source: crate::ui::terminal::Error },

    #[snafu(display("{source}"))]
    Configuration { source: crate::config::Error },

    #[snafu(display("Spec {spec_name} is not found"))]
    SpecNotFound { spec_name: String },

    #[snafu(display("Failed to write stdout, error: {source}"))]
    WriteStdout { source: std::io::Error },

    #[snafu(display("Failed to initialize Kubernetes client, error: {source}"))]
    KubeConfig { source: kube::Error },

    #[snafu(display("Failed to create pod {pod_name} in namespace {namespace}, error: {source}"))]
    CreatePod {
        namespace: String,
        pod_name: String,
        #[snafu(source(from(kube::Error, Box::new)))]
        source: Box<kube::Error>,
    },

    #[snafu(display("Failed to delete pod {pod_name} in namespace {namespace}, error: {source}"))]
    DeletePod {
        namespace: String,
        pod_name: String,
        #[snafu(source(from(kube::Error, Box::new)))]
        source: Box<kube::Error>,
    },

    #[snafu(display("Failed to attach pod {pod_name} in namespace {namespace}, error: {source}"))]
    AttachPod {
        namespace: String,
        pod_name: String,
        #[snafu(source(from(kube::Error, Box::new)))]
        source: Box<kube::Error>,
    },

    #[snafu(display("Failed to list pods, error: {source}"))]
    ListPods {
        #[snafu(source(from(kube::Error, Box::new)))]
        source: Box<kube::Error>,
    },

    #[snafu(display(
        "Failed to get pod {pod_name} status in namespace {namespace}, error: {source}"
    ))]
    GetPod {
        namespace: String,
        pod_name: String,
        #[snafu(source(from(kube::Error, Box::new)))]
        source: Box<kube::Error>,
    },

    #[snafu(display("Failed to wait for pod {pod_name} status in namespace {namespace}"))]
    WaitForPodStatus { namespace: String, pod_name: String },

    #[snafu(display(
        "Failed to wait for pod {pod_name} status in namespace {namespace}, error: {source}"
    ))]
    GetPodStatus {
        namespace: String,
        pod_name: String,
        #[snafu(source(from(kube::runtime::wait::Error, Box::new)))]
        source: Box<kube::runtime::wait::Error>,
    },

    #[snafu(display("Failed to list pods in namespace {namespace}, error: {source}"))]
    ListPodsWithNamespace {
        namespace: String,
        #[snafu(source(from(kube::Error, Box::new)))]
        source: Box<kube::Error>,
    },

    #[snafu(display("Could not create tokio runtime, error: {source}"))]
    InitializeTokioRuntime { source: std::io::Error },

    #[snafu(display("Error occurs while copying I/O bidirectionally, error: {source}"))]
    CopyBidirectionalIo { source: std::io::Error },

    #[snafu(display("{stream} requested but missing"))]
    GetPodStream { stream: &'static str },

    #[snafu(display("Failed to get terminal size, error: {source}"))]
    GetTerminalSize { source: std::io::Error },

    #[snafu(display("Failed to change terminal size"))]
    ChangeTerminalSize,

    #[snafu(display("Failed to create signal stream, error: {source}"))]
    CreateSignalStream { source: std::io::Error },

    #[snafu(display("Failed to get terminal size writer"))]
    GetTerminalSizeWriter,

    #[snafu(display("Failed to bind TCP socket {socket_address}, error: {source}"))]
    BindTcpSocket { socket_address: SocketAddr, source: std::io::Error },

    #[snafu(display("Failed to accept TCP socket {socket_address}, error: {source}"))]
    AcceptTcpSocket { socket_address: SocketAddr, source: std::io::Error },

    #[snafu(display("Failed to create pod stream {stream_id}, error: {source}"))]
    CreatePodStream {
        stream_id: String,
        #[snafu(source(from(kube::Error, Box::new)))]
        source: Box<kube::Error>,
    },

    #[snafu(display("Failed to upload or authorize the SSH key in the pod {pod_name}: {source}"))]
    UploadSshKey {
        namespace: String,
        pod_name: String,
        #[snafu(source(from(kube::Error, Box::new)))]
        source: Box<kube::Error>,
    },

    #[snafu(display("No SSH private key is provided"))]
    NoSshPrivateKeyProvided,
}

impl From<crate::ssh::Error> for Error {
    fn from(source: crate::ssh::Error) -> Self { Self::Ssh { source } }
}

impl From<crate::ui::terminal::Error> for Error {
    fn from(source: crate::ui::terminal::Error) -> Self { Self::TerminalUi { source } }
}

impl From<crate::config::Error> for Error {
    fn from(source: crate::config::Error) -> Self { Self::Configuration { source } }
}
