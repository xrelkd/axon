use std::{borrow::Cow, path::PathBuf};

use snafu::Snafu;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    #[snafu(display("Operation has been cancelled"))]
    Cancelled,

    #[snafu(display("No SSH private key is provided"))]
    NoSshPrivateKeyProvided,

    #[snafu(display("Failed to read the local SSH private key file {}, error: {source}", file_path.display()))]
    ReadSshPrivateKey { file_path: PathBuf, source: std::io::Error },

    #[snafu(display("Failed to parse SSH private key"))]
    ParseSshPrivateKey,

    #[snafu(display("Failed to serialize SSH public key"))]
    SerializeSshPublicKey,

    #[snafu(display("Failed to connect to the SSH server: {source}"))]
    ConnectServer { source: russh::Error },

    #[snafu(display("Failed to authenticate user {user}: {source}"))]
    AuthenticateUser { user: String, source: russh::Error },

    #[snafu(display("Access denied for user {user}"))]
    DenyAccess { user: String },

    #[snafu(display("Failed to open a new SSH session channel: {source}"))]
    OpenChannel { source: russh::Error },

    #[snafu(display("Failed to request a PTY (pseudo-terminal): {source}"))]
    RequestPty { source: russh::Error },

    #[snafu(display("Failed to execute command: {source}"))]
    ExecuteCommand { source: russh::Error },

    #[snafu(display("Failed to send data over the SSH channel: {source}"))]
    SendChannelData { source: russh::Error },

    #[snafu(display("Failed to close the SSH channel (EOF): {source}"))]
    CloseChannel { source: russh::Error },

    #[snafu(display("Failed to determine terminal size: {source}"))]
    GetTerminalSize { source: std::io::Error },

    #[snafu(display("Failed to initialize standard I/O stream '{stream}', error: {source}"))]
    InitializeStdio { stream: Cow<'static, str>, source: std::io::Error },

    #[snafu(display("Failed to write to local stdout: {source}"))]
    WriteStdout { source: std::io::Error },

    #[snafu(display("Failed to read from standard input: {source}"))]
    ReadStdin { source: std::io::Error },

    #[snafu(display("Failed to disconnect session: {source}"))]
    DisconnectSession { source: russh::Error },

    #[snafu(display("Failed to open SFTP subsystem: {source}"))]
    OpenSftp { source: russh::Error },

    #[snafu(display("Failed to open SFTP session, error: {source}"))]
    OpenSftpSession { source: russh_sftp::client::error::Error },

    #[snafu(display("Failed to open local file '{}': {source}", path.display()))]
    OpenLocalFile { path: PathBuf, source: std::io::Error },

    #[snafu(display("Failed to open remote file '{path}', error: {source}"))]
    OpenRemoteFile { path: String, source: russh_sftp::client::error::Error },

    #[snafu(display("Failed to transfer data for '{}', error: {source}", path.display()))]
    TransferData { path: PathBuf, source: std::io::Error },
}
