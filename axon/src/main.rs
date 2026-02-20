//! The `axon` crate provides a command-line interface (CLI) for managing and
//! interacting with various system components. It offers functionalities such
//! as CLI command execution, configuration management, and SSH capabilities.

mod cli;
mod config;
mod consts;
mod ext;
mod pod_console;
mod port_forwarder;
mod ssh;
mod ui;

/// This module provides build-time information for the application,
/// utilizing the `shadow-rs` crate to embed details such as the
/// build version, commit hash, and build date.
mod shadow {
    #![allow(clippy::needless_raw_string_hashes)]
    use shadow_rs::shadow;
    shadow!(build);

    pub use self::build::*;
}

use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

use directories::ProjectDirs;

use self::cli::Cli;

/// The version of the project, sourced from `CARGO_PKG_VERSION` at compile
/// time.
pub const PROJECT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// A lazily initialized `semver::Version` representation of the project's
/// version.
///
/// If `PROJECT_VERSION` cannot be parsed as a valid semver, it defaults to
/// `0.0.0`.
pub static PROJECT_SEMVER: LazyLock<semver::Version> = LazyLock::new(|| {
    semver::Version::parse(PROJECT_VERSION).unwrap_or(semver::Version {
        major: 0,
        minor: 0,
        patch: 0,
        pre: semver::Prerelease::EMPTY,
        build: semver::BuildMetadata::EMPTY,
    })
});

/// The name of the project in lowercase.
pub const PROJECT_NAME: &str = "axon";
/// The name of the project with its initial letter capitalized.
pub const PROJECT_NAME_WITH_INITIAL_CAPITAL: &str = "Axon";
/// The summary text used for notifications related to Axon.
pub const NOTIFICATION_SUMMARY: &str = "Axon";

/// The program name used for CLI execution.
pub const CLI_PROGRAM_NAME: &str = "axon";
/// The default filename for the CLI configuration.
pub const CLI_CONFIG_NAME: &str = "config.yaml";

/// The default prompt text displayed in menus or interactive selections.
pub const DEFAULT_MENU_PROMPT: &str = "Axon";

/// A `PathBuf` representing the project's configuration
/// directory. This path is determined using `directories::ProjectDirs` to
/// ensure OS-specific and user-specific conventions are followed.
///
/// # Panics
/// This constant uses `expect()` internally during initialization. If
/// `ProjectDirs::from` fails to determine the appropriate project directories
/// (which is highly unlikely in a typical operating environment), the
/// application will panic.
pub static PROJECT_CONFIG_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    ProjectDirs::from("", PROJECT_NAME, PROJECT_NAME)
        .expect("Creating `ProjectDirs` should always success")
        .config_dir()
        .to_path_buf()
});

/// The fallback project configuration directories.
///
/// Returns a list of fallback directories where project configuration files
/// might be located. These paths are typically used if the primary
/// configuration directory (`PROJECT_CONFIG_DIR`) does not exist or does not
/// contain the necessary files.
///
/// The fallback directories include:
/// 1. `$HOME/.config/axon`
/// 2. `$HOME/.axon`
///
/// # Returns
/// A `Vec<PathBuf>` containing potential fallback configuration directory
/// paths. Returns an empty vector if `directories::UserDirs::new()` fails to
/// retrieve user directories.
///
/// # Examples
/// ```rust
/// use std::path::PathBuf;
/// use axon::fallback_project_config_directories;
///
/// let fallback_dirs = fallback_project_config_directories();
/// // On a Linux system, this might produce paths like:
/// // - /home/user/.config/axon
/// // - /home/user/.axon
/// // On other OS, the paths would conform to their respective conventions.
/// assert!(fallback_dirs.iter().any(|p| p.ends_with(".config/axon") || p.ends_with(".axon")));
/// ```
#[must_use]
pub fn fallback_project_config_directories() -> Vec<PathBuf> {
    let Some(user_dirs) = directories::UserDirs::new() else {
        return Vec::new();
    };
    vec![
        [user_dirs.home_dir(), Path::new(".config"), Path::new(PROJECT_NAME)].iter().collect(),
        [user_dirs.home_dir(), Path::new(&format!(".{PROJECT_NAME}"))].iter().collect(),
    ]
}

/// The main entry point for the Axon CLI application.
///
/// This function parses command-line arguments, executes the requested command,
/// and handles any errors that occur during execution. It exits the process
/// with an appropriate status code (0 for success, 1 for error).
///
/// # Errors
/// If the `Cli::run()` method returns an `Err`, an error message is printed
/// to `stderr`, and the process exits with a status code of 1.
fn main() {
    match Cli::default().run() {
        Ok(exit_code) => {
            std::process::exit(exit_code);
        }
        Err(err) => {
            eprintln!("Error: {err}");
            std::process::exit(1);
        }
    }
}
