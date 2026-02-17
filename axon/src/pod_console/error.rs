use std::borrow::Cow;

use snafu::Snafu;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    #[snafu(display("{source}"))]
    TerminalUi { source: crate::ui::terminal::Error },

    #[snafu(display("Failed to attach pod {pod_name} in namespace {namespace}, error: {source}"))]
    AttachPod {
        namespace: String,
        pod_name: String,
        #[snafu(source(from(kube::Error, Box::new)))]
        source: Box<kube::Error>,
    },

    #[snafu(display("Failed to initialize standard I/O streams ({stream}), error: {source}"))]
    InitializeStdio { stream: Cow<'static, str>, source: std::io::Error },

    #[snafu(display("Error occurs while copying I/O, error: {source}"))]
    CopyIo { source: std::io::Error },

    #[snafu(display("{stream} requested but missing"))]
    GetPodStream { stream: Cow<'static, str> },

    #[snafu(display("Failed to get terminal size, error: {source}"))]
    GetTerminalSize { source: std::io::Error },

    #[snafu(display("Failed to change terminal size"))]
    ChangeTerminalSize,

    #[snafu(display("Failed to create signal stream, error: {source}"))]
    CreateSignalStream { source: std::io::Error },

    #[snafu(display("Failed to get terminal size writer"))]
    GetTerminalSizeWriter,
}

impl From<crate::ui::terminal::Error> for Error {
    fn from(source: crate::ui::terminal::Error) -> Self { Self::TerminalUi { source } }
}
