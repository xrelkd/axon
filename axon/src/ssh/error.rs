//! This module defines the error types that can occur during SSH operations
//! within the application.

use std::{borrow::Cow, path::PathBuf};

use snafu::Snafu;

/// Represents the various errors that can occur during SSH operations,
/// including connection issues, authentication failures, channel management,
/// command execution, and SFTP transfers.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    /// The SSH operation was cancelled.
    #[snafu(display("Operation has been cancelled"))]
    Cancelled,

    /// No SSH private key was provided for authentication.
    #[snafu(display("No SSH private key was provided for authentication"))]
    NoSshPrivateKeyProvided,

    /// Failed to resolve any valid SSH identity.
    #[snafu(
        display(
            "Failed to resolve any valid SSH identity. Attempted paths: [{}], error: {source}",
            paths.iter().map(|path| path.display().to_string()).collect::<Vec<_>>().join(", ")
        )
    )]
    ResolveIdentities {
        paths: Vec<PathBuf>,

        #[allow(clippy::use_self)]
        source: Box<Error>,
    },

    /// Failed to read the local SSH private key file.
    ///
    /// # Fields
    /// - `file_path`: The path to the private key file that could not be read.
    /// - `source`: The underlying `std::io::Error`.
    #[snafu(display("Failed to read the local SSH private key file {}, error: {source}", file_path.display()))]
    ReadSshPrivateKey { file_path: PathBuf, source: std::io::Error },

    /// Failed to parse the provided SSH private key.
    ///
    /// This typically indicates an invalid key format.
    #[snafu(display("Failed to parse SSH private key"))]
    ParseSshPrivateKey,

    /// Failed to serialize the SSH public key.
    #[snafu(display("Failed to serialize SSH public key"))]
    SerializeSshPublicKey,

    /// Failed to connect to the SSH server.
    ///
    /// # Fields
    /// - `source`: The underlying `russh::Error` indicating the connection
    ///   failure.
    #[snafu(display("Failed to connect to the SSH server, error: {source}"))]
    ConnectServer { source: russh::Error },

    /// Failed to authenticate the user with the SSH server.
    ///
    /// # Fields
    /// - `user`: The username that failed to authenticate.
    /// - `source`: The underlying `russh::Error` indicating the authentication
    ///   failure.
    #[snafu(display("Failed to authenticate user {user}, error: {source}"))]
    AuthenticateUser { user: String, source: russh::Error },

    /// Access denied for the specified user.
    ///
    /// This error typically occurs after authentication, indicating that the
    /// user does not have permission to perform the requested action.
    ///
    /// # Fields
    /// - `user`: The username for whom access was denied.
    #[snafu(display("Access denied for user {user}"))]
    DenyAccess { user: String },

    /// Failed to open a new SSH session channel.
    ///
    /// # Fields
    /// - `source`: The underlying `russh::Error`.
    #[snafu(display("Failed to open a new SSH session channel, error: {source}"))]
    OpenChannel { source: russh::Error },

    /// Failed to request a PTY (pseudo-terminal) for the SSH session.
    ///
    /// # Fields
    /// - `source`: The underlying `russh::Error`.
    #[snafu(display("Failed to request a PTY (pseudo-terminal), error: {source}"))]
    RequestPty { source: russh::Error },

    /// Failed to execute a command over SSH.
    ///
    /// # Fields
    /// - `source`: The underlying `russh::Error`.
    #[snafu(display("Failed to execute command, error: {source}"))]
    ExecuteCommand { source: russh::Error },

    /// Failed to send data over the SSH channel.
    ///
    /// # Fields
    /// - `source`: The underlying `russh::Error`.
    #[snafu(display("Failed to send data over the SSH channel, error: {source}"))]
    SendChannelData { source: russh::Error },

    /// Failed to close the SSH channel (EOF).
    ///
    /// # Fields
    /// - `source`: The underlying `russh::Error`.
    #[snafu(display("Failed to close the SSH channel (EOF), error: {source}"))]
    CloseChannel { source: russh::Error },

    /// Failed to determine the terminal size.
    ///
    /// This error occurs when attempting to get the dimensions of the local
    /// terminal.
    ///
    /// # Fields
    /// - `source`: The underlying `std::io::Error`.
    #[snafu(display("Failed to determine terminal size, error: {source}"))]
    GetTerminalSize { source: std::io::Error },

    /// Failed to initialize a standard I/O stream (e.g., stdin, stdout,
    /// stderr).
    ///
    /// # Fields
    /// - `stream`: The name of the stream that failed to initialize (e.g.,
    ///   "stdin").
    /// - `source`: The underlying `std::io::Error`.
    #[snafu(display("Failed to initialize standard I/O stream '{stream}', error: {source}"))]
    InitializeStdio { stream: Cow<'static, str>, source: std::io::Error },

    /// Failed to write data to local standard output.
    ///
    /// # Fields
    /// - `source`: The underlying `std::io::Error`.
    #[snafu(display("Failed to write to local stdout, error: {source}"))]
    WriteStdout { source: std::io::Error },

    /// Failed to read data from standard input.
    ///
    /// # Fields
    /// - `source`: The underlying `std::io::Error`.
    #[snafu(display("Failed to read from standard input, error: {source}"))]
    ReadStdin { source: std::io::Error },

    /// Failed to disconnect the SSH session.
    ///
    /// # Fields
    /// - `source`: The underlying `russh::Error`.
    #[snafu(display("Failed to disconnect session, error: {source}"))]
    DisconnectSession { source: russh::Error },

    /// Failed to open the SFTP subsystem.
    ///
    /// # Fields
    /// - `source`: The underlying `russh::Error`.
    #[snafu(display("Failed to open SFTP subsystem, error: {source}"))]
    OpenSftp { source: russh::Error },

    /// Failed to open an SFTP session.
    ///
    /// # Fields
    /// - `source`: The underlying `russh_sftp::client::error::Error`.
    #[snafu(display("Failed to open SFTP session, error: {source}"))]
    OpenSftpSession { source: russh_sftp::client::error::Error },

    /// Failed to open a local file for SFTP transfer.
    ///
    /// # Fields
    /// - `path`: The path to the local file that could not be opened.
    /// - `source`: The underlying `std::io::Error`.
    #[snafu(display("Failed to open local file '{}', error: {source}", path.display()))]
    OpenLocalFile { path: PathBuf, source: std::io::Error },

    /// Failed to open a remote file for SFTP transfer.
    ///
    /// # Fields
    /// - `path`: The path to the remote file that could not be opened.
    /// - `source`: The underlying `russh_sftp::client::error::Error`.
    #[snafu(display("Failed to open remote file '{path}', error: {source}"))]
    OpenRemoteFile { path: String, source: russh_sftp::client::error::Error },

    /// Failed to transfer data for a file during SFTP.
    ///
    /// This could occur during reading from a local file or writing to a remote
    /// file.
    ///
    /// # Fields
    /// - `path`: The path to the file involved in the failed transfer.
    /// - `source`: The underlying `std::io::Error`.
    #[snafu(display("Failed to transfer data for '{}', error: {source}", path.display()))]
    TransferData { path: PathBuf, source: std::io::Error },
}
