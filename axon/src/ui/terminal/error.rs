use snafu::Snafu;

/// Represents errors that can occur during terminal operations.
///
/// This enum encapsulates various issues that might arise when interacting with
/// the terminal, such as failures to set terminal modes.
///
/// # Examples
///
/// ```rust
/// use std::io;
/// # use snafu::Snafu;
/// #
/// # #[derive(Debug, Snafu)]
/// # #[snafu(visibility(pub))]
/// # pub enum Error {
/// #     #[snafu(display("Failed to enable terminal raw mode, error: {source}"))]
/// #     EnableTerminalRawMode { source: std::io::Error },
/// # }
///
/// // Simulate an I/O error that might occur when trying to enable raw mode
/// let io_error = io::Error::new(io::ErrorKind::PermissionDenied, "Operation not permitted");
///
/// // Construct the specific error variant
/// let terminal_error = Error::EnableTerminalRawMode { source: io_error };
///
/// println!("Encountered terminal error: {}", terminal_error);
/// assert!(matches!(terminal_error, Error::EnableTerminalRawMode { .. }));
/// ```
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    /// Error returned when failing to enable terminal raw mode.
    ///
    /// This error typically occurs when the underlying terminal device
    /// does not support raw mode, or there are permission issues
    /// preventing the application from changing terminal settings.
    ///
    /// # Fields
    ///
    /// * `source` - The underlying `std::io::Error` that caused this error,
    ///   providing more specific details about the failure.
    #[snafu(display("Failed to enable terminal raw mode, error: {source}"))]
    EnableTerminalRawMode { source: std::io::Error },
}
