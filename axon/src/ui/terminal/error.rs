//! Terminal operation error types.
//!
//! This module defines the [`Error`] enum for terminal-related failures,
//! such as failures to enable raw mode.

//! Terminal operation error types.
//!
//! This module defines the [`Error`] enum for terminal-related failures,
//! such as failures to enable raw mode.

use snafu::Snafu;

/// Represents errors that can occur during terminal operations.
///
/// This enum encapsulates various issues that might arise when interacting with
/// the terminal, such as failures to set terminal modes.
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
