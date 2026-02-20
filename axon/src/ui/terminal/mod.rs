mod error;

use std::io::Write;

use snafu::ResultExt;

pub use self::error::Error;

/// A guard that ensures the terminal raw mode is properly enabled and disabled.
///
/// When an instance of `TerminalRawModeGuard` is created using `setup()`,
/// it enables raw mode for the terminal. Upon the instance being dropped,
/// it automatically disables raw mode and attempts to restore the terminal
/// to its previous state, including writing a carriage return and flushing
/// standard output. This is crucial for maintaining a clean terminal state
/// after operations that require raw mode, even if the program exits
/// unexpectedly.
pub struct TerminalRawModeGuard;

impl TerminalRawModeGuard {
    /// Sets up the terminal by enabling raw mode.
    ///
    /// This function enables raw mode using
    /// `crossterm::terminal::enable_raw_mode()`. The returned
    /// `TerminalRawModeGuard` acts as a RAII guard; when it is dropped, raw
    /// mode will be disabled.
    ///
    /// # Errors
    ///
    /// Returns an `Error` if `crossterm::terminal::enable_raw_mode()` fails,
    /// typically due to an underlying I/O error when interacting with the
    /// terminal.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use axon::ui::terminal::TerminalRawModeGuard;
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let _guard = TerminalRawModeGuard::setup()?;
    ///     // Terminal is in raw mode here
    ///     println!("Terminal is in raw mode. Press any key to continue...");
    ///     std::io::stdin().read_line(&mut String::new())?;
    ///     // When _guard goes out of scope, raw mode is disabled
    ///     Ok(())
    /// }
    /// ```
    pub fn setup() -> Result<Self, Error> {
        crossterm::terminal::enable_raw_mode().context(error::EnableTerminalRawModeSnafu)?;
        Ok(Self)
    }
}

impl Drop for TerminalRawModeGuard {
    /// Disables terminal raw mode and restores standard output upon dropping
    /// the guard.
    ///
    /// This implementation ensures that
    /// `crossterm::terminal::disable_raw_mode()` is called, and a carriage
    /// return (`\r`) is written to standard output, followed by a flush.
    /// This helps in cleaning up the terminal's state after raw mode
    /// operations, making sure the cursor is at the beginning of the line
    /// and any buffered output is displayed.
    ///
    /// Any errors encountered during `disable_raw_mode` or writing to stdout
    /// are ignored, as `drop` implementations should not panic.
    fn drop(&mut self) {
        let _unused = crossterm::terminal::disable_raw_mode();

        let mut stdout = std::io::stdout().lock();
        let _unused = stdout.write_all(b"\r");
        let _unused = stdout.flush();
    }
}
