mod error;

use std::io::Write;

use snafu::ResultExt;

pub use self::error::Error;

// Helper to ensure terminal is restored if the program exits
pub struct TerminalRawModeGuard;

impl TerminalRawModeGuard {
    pub fn setup() -> Result<Self, Error> {
        crossterm::terminal::enable_raw_mode().context(error::EnableTerminalRawModeSnafu)?;
        Ok(Self)
    }
}

impl Drop for TerminalRawModeGuard {
    fn drop(&mut self) {
        let _unused = crossterm::terminal::disable_raw_mode();

        let mut stdout = std::io::stdout().lock();
        let _unused = stdout.write_all(b"\r");
        let _unused = stdout.flush();
    }
}
