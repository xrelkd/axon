//! Defines the error types for the pod console operations.
//!
//! This module centralizes error handling for various issues that can arise
//! during interaction with pod consoles, including terminal UI problems,
//! attachment failures, I/O errors, and terminal size management issues.

use std::borrow::Cow;

use snafu::Snafu;

/// Represents the various errors that can occur during pod console operations.
///
/// This enum encapsulates errors related to terminal UI interactions, attaching
/// to Kubernetes pods, standard I/O management, and terminal resizing.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    /// An error occurred within the terminal UI component.
    #[snafu(display("{source}"))]
    TerminalUi { source: crate::ui::terminal::Error },

    /// Failed to attach to a Kubernetes pod.
    ///
    /// This error typically occurs when there are issues connecting to the pod
    /// or when the Kubernetes API server reports an error during the attachment
    /// process.
    #[snafu(display("Failed to attach pod {pod_name} in namespace {namespace}, error: {source}"))]
    AttachPod {
        /// The namespace of the pod that failed to attach.
        namespace: String,
        /// The name of the pod that failed to attach.
        pod_name: String,
        #[snafu(source(from(kube::Error, Box::new)))]
        /// The underlying `kube::Error` that caused the attachment failure.
        source: Box<kube::Error>,
    },

    /// Failed to initialize a standard I/O stream (e.g., stdin, stdout,
    /// stderr).
    #[snafu(display("Failed to initialize standard I/O stream '{stream}', error: {source}"))]
    InitializeStdio {
        /// The name of the stream that failed to initialize (e.g., "stdin").
        stream: Cow<'static, str>,
        /// The underlying `std::io::Error`.
        source: std::io::Error,
    },

    /// Failed to copy data between I/O streams.
    #[snafu(display("Failed to copy I/O, error: {source}"))]
    CopyIo { source: std::io::Error },

    /// The requested pod stream (e.g., stdin, stdout, stderr) was missing or
    /// not available from the pod's attachment process.
    #[snafu(display("Requested pod stream '{stream}' is missing"))]
    GetPodStream {
        /// The name of the stream that was missing.
        stream: Cow<'static, str>,
    },

    /// Failed to retrieve the current terminal size.
    #[snafu(display("Failed to get terminal size, error: {source}"))]
    GetTerminalSize { source: std::io::Error },

    /// Failed to change the terminal size (e.g., due to an OS error or an
    /// invalid size).
    #[snafu(display("Failed to change terminal size"))]
    ChangeTerminalSize,

    /// Failed to create a signal stream, which is used for handling terminal
    /// resizing signals.
    #[snafu(display("Failed to create signal stream, error: {source}"))]
    CreateSignalStream { source: std::io::Error },

    /// Failed to obtain a writer for setting the terminal size.
    #[snafu(display("Failed to obtain terminal size writer"))]
    GetTerminalSizeWriter,
}

impl From<crate::ui::terminal::Error> for Error {
    /// Converts a `crate::ui::terminal::Error` into a
    /// `pod_console::Error::TerminalUi`.
    ///
    /// This allows `terminal::Error` instances to be seamlessly integrated into
    /// the pod console's error handling.
    ///
    /// # Arguments
    ///
    /// * `source` - The `crate::ui::terminal::Error` to convert.
    ///
    /// # Returns
    ///
    /// A new `Error::TerminalUi` variant containing the original
    /// `terminal::Error`.
    fn from(source: crate::ui::terminal::Error) -> Self { Self::TerminalUi { source } }
}
