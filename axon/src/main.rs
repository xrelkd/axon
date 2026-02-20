mod cli;
mod config;
mod consts;
mod ext;
mod pod_console;
mod port_forwarder;
mod ssh;
mod ui;
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

pub const PROJECT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub static PROJECT_SEMVER: LazyLock<semver::Version> = LazyLock::new(|| {
    semver::Version::parse(PROJECT_VERSION).unwrap_or(semver::Version {
        major: 0,
        minor: 0,
        patch: 0,
        pre: semver::Prerelease::EMPTY,
        build: semver::BuildMetadata::EMPTY,
    })
});

pub const PROJECT_NAME: &str = "axon";
pub const PROJECT_NAME_WITH_INITIAL_CAPITAL: &str = "Axon";
pub const NOTIFICATION_SUMMARY: &str = "Axon";

pub const CLI_PROGRAM_NAME: &str = "axon";
pub const CLI_CONFIG_NAME: &str = "config.yaml";

pub const DEFAULT_MENU_PROMPT: &str = "Axon";

pub static PROJECT_CONFIG_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    ProjectDirs::from("", PROJECT_NAME, PROJECT_NAME)
        .expect("Creating `ProjectDirs` should always success")
        .config_dir()
        .to_path_buf()
});

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
