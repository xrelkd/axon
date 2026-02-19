use snafu::Snafu;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    #[snafu(display("{message}"))]
    Generic { message: String },

    #[snafu(display("{source}"))]
    Configuration { source: crate::config::Error },

    #[snafu(display("{source}"))]
    Ssh { source: crate::ssh::Error },

    #[snafu(display("{source}"))]
    TerminalUi { source: crate::ui::terminal::Error },

    #[snafu(display("{source}"))]
    PortForwarder { source: crate::port_forwarder::Error },

    #[snafu(display("{source}"))]
    PodConsole { source: crate::pod_console::Error },

    #[snafu(display("Image specification '{spec_name}' not found"))]
    SpecNotFound { spec_name: String },

    #[snafu(display("Failed to write to stdout, error: {source}"))]
    WriteStdout { source: std::io::Error },

    #[snafu(display("Failed to initialize Kubernetes client configuration, error: {source}"))]
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

    #[snafu(display(
        "Timed out waiting for pod '{pod_name}' to reach running status in namespace '{namespace}'"
    ))]
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

    #[snafu(display("Failed to create tokio runtime, error: {source}"))]
    InitializeTokioRuntime { source: std::io::Error },

    #[snafu(display("Failed to upload or authorize SSH key in pod '{pod_name}', error: {source}"))]
    UploadSshKey {
        namespace: String,
        pod_name: String,
        #[snafu(source(from(kube::Error, Box::new)))]
        source: Box<kube::Error>,
    },

    #[snafu(display("No SSH private key is provided"))]
    NoSshPrivateKeyProvided,

    #[snafu(display("Failed to serialize interactive shell configuration, error: {source}"))]
    SerializeInteractiveShell { source: serde_json::Error },
}

impl From<crate::config::Error> for Error {
    fn from(source: crate::config::Error) -> Self { Self::Configuration { source } }
}

impl From<crate::ssh::Error> for Error {
    fn from(source: crate::ssh::Error) -> Self { Self::Ssh { source } }
}

impl From<crate::ui::terminal::Error> for Error {
    fn from(source: crate::ui::terminal::Error) -> Self { Self::TerminalUi { source } }
}

impl From<crate::port_forwarder::Error> for Error {
    fn from(source: crate::port_forwarder::Error) -> Self { Self::PortForwarder { source } }
}

impl From<crate::pod_console::Error> for Error {
    fn from(source: crate::pod_console::Error) -> Self { Self::PodConsole { source } }
}
