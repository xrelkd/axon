use std::io::Write;

use snafu::ResultExt;

use crate::{error, error::Error};

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
        let _ = std::io::stdout().flush().ok();
        let _unused = crossterm::terminal::disable_raw_mode();
        let mut stdout = std::io::stdout().lock();
        let _ = stdout.write_all(b"\r").ok();
        let _ = stdout.flush().ok();
    }
}
