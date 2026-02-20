//! The `axon` crate provides a robust Command Line Interface (CLI) for advanced
//! Kubernetes resource management.
//!
//! It offers extended functionality for common Kubernetes operational tasks,
//! including pod management, image handling, and secure shell access.
//!
//! # Examples
//!
//! ```bash
//! # List all temporary pods managed by Axon
//! axon list
//!
//! # Create a new temporary pod with a specific image
//! axon create --image my-repo/my-image:latest
//!
//! # Attach to a running pod's console
//! axon attach my-pod-name
//!
//! # Execute a command inside a pod
//! axon execute my-pod-name -- ls -la /app
//!
//! # Forward a local port to a pod port
//! axon port-forward my-pod-name 8080:80
//! ```

mod attach;
mod create;
mod delete;
pub mod error;
mod execute;
mod image;
mod internal;
mod list;
mod port_forward;
mod ssh;

use std::{io::Write, path::PathBuf};

use clap::{CommandFactory, Parser, Subcommand};
use futures::FutureExt;
use snafu::ResultExt;
use tokio::runtime::Runtime;

pub use self::error::Error;
use self::{
    attach::AttachCommand, create::CreateCommand, delete::DeleteCommand, execute::ExecuteCommand,
    image::ImageCommands, list::ListCommand, port_forward::PortForwardCommand, ssh::SshCommands,
};
use crate::{CLI_PROGRAM_NAME, config::Config, shadow};

/// `Cli` is the main entry point for the Axon Command Line Interface.
///
/// It parses command-line arguments and dispatches to appropriate subcommands
/// for Kubernetes resource management.
#[derive(Parser)]
#[command(
    name = CLI_PROGRAM_NAME,
    author,
    version,
    long_version = shadow::CLAP_LONG_VERSION,
    about = "Axon CLI: A robust tool for advanced Kubernetes resource management.",
    long_about = "Axon is a powerful Rust-based Command Line Interface (CLI) tool \
                  designed for advanced interaction with Kubernetes resources. It \
                  provides extended functionality and a specialized interface for \
                  common Kubernetes operational tasks, including pod management, \
                  image handling, and secure shell access.",
    color = clap::ColorChoice::Always
)]
pub struct Cli {
    /// The subcommand to execute.
    #[clap(subcommand)]
    commands: Option<Commands>,

    /// Path to the configuration file.
    ///
    /// Defaults to `~/.config/axon/config.yaml` or the path specified by the
    /// `AXON_CONFIG_FILE_PATH` environment variable.
    #[clap(
        long = "config",
        short = 'c',
        env = "AXON_CONFIG_FILE_PATH",
        help = "Specify a configuration file. Defaults to ~/.config/axon/config.yaml or \
                AXON_CONFIG_FILE_PATH env var."
    )]
    config_file: Option<PathBuf>,

    /// Sets the logging level for the application.
    ///
    /// Supported levels include `info`, `debug`, and `trace`.
    #[clap(
        long = "log-level",
        env = "AXON_LOG_LEVEL",
        help = "Set the logging level (e.g., info, debug, trace)."
    )]
    log_level: Option<tracing::Level>,
}

/// `Commands` enumerates the available subcommands for the Axon CLI.
///
/// Each variant corresponds to a specific operation or category of operations
/// within Kubernetes.
#[derive(Clone, Subcommand)]
pub enum Commands {
    /// Displays client and server version information.
    #[command(about = "Display client and server version information")]
    Version {
        /// If true, shows only the client version and does not require a server
        /// connection.
        #[clap(long = "client", help = "If true, shows client version only (no server required).")]
        client: bool,
    },

    /// Generates a shell completion script for the specified shell.
    ///
    /// # Arguments
    ///
    /// * `shell` - The shell for which to generate completions (e.g., `bash`,
    ///   `zsh`, `fish`).
    #[command(about = "Generate shell completion script for the specified shell (bash, zsh, fish)")]
    Completions { shell: clap_complete::Shell },

    /// Outputs the default configuration in YAML format to standard output.
    #[command(about = "Output the default configuration in YAML format")]
    DefaultConfig,

    /// Creates a new temporary pod in a specified namespace or using a
    /// predefined spec.
    #[command(
        alias = "c",
        about = "Create a new temporary pod in a specified namespace or using a predefined spec"
    )]
    Create(CreateCommand),

    /// Deletes one or more temporary pods managed by Axon.
    #[command(alias = "d", about = "Delete one or more temporary pods managed by Axon")]
    Delete(DeleteCommand),

    /// Attaches to a running temporary pod's console.
    #[command(alias = "a", about = "Attach to a running temporary pod's console")]
    Attach(AttachCommand),

    /// Executes a command inside a running temporary pod.
    #[command(
        aliases = ["e", "exec"],
        about = "Execute a command inside a running temporary pod"
    )]
    Execute(ExecuteCommand),

    /// Lists all temporary pods currently managed by Axon.
    #[command(alias = "l", about = "List all temporary pods managed by Axon")]
    List(ListCommand),

    /// Forwards one or more local ports to a specific port on a temporary pod.
    #[command(
        aliases = ["p", "pf"],
        about = "Forward one or more local ports to a specific port on a temporary pod"
    )]
    PortForward(PortForwardCommand),

    /// Manages container image specifications.
    #[command(alias = "i", about = "Manage container image specifications")]
    Image {
        /// Subcommands for image management (e.g., `list`).
        #[command(subcommand)]
        commands: ImageCommands,
    },

    /// Securely interacts with a temporary pod via SSH, supporting shell
    /// access, file transfer, and setup.
    #[command(
        about = "Securely interact with a temporary pod via SSH (shell, file transfer, setup)"
    )]
    Ssh {
        /// Subcommands for SSH operations (e.g., `shell`, `get`, `put`).
        #[command(subcommand)]
        commands: SshCommands,
    },
}

impl Default for Cli {
    /// Creates a new `Cli` instance by parsing command-line arguments.
    ///
    /// This method uses `clap::Parser::parse()` to initialize the `Cli` struct.
    fn default() -> Self { Self::parse() }
}

impl Cli {
    /// Loads the application configuration, applying any overrides from CLI
    /// arguments.
    ///
    /// If a configuration file path is provided via the `--config` flag or
    /// `AXON_CONFIG_FILE_PATH` environment variable, it is used. Otherwise,
    /// Axon searches for a default configuration file. The `log_level` from
    /// CLI arguments (if present) overrides the configuration file's setting.
    ///
    /// # Errors
    ///
    /// Returns an `Error` if:
    /// - The configuration file cannot be loaded or parsed.
    /// - There are issues searching for the default configuration file path.
    ///
    /// # Returns
    ///
    /// A `Result` containing the loaded and potentially overridden `Config` on
    /// success, or an `Error` if any step fails.
    fn load_config(&self) -> Result<Config, Error> {
        let mut config =
            Config::load(self.config_file.clone().unwrap_or_else(Config::search_config_file_path))?;

        if let Some(log_level) = self.log_level {
            config.log.level = log_level;
        }

        Ok(config)
    }

    /// Executes the main logic of the CLI application based on the parsed
    /// command and arguments.
    ///
    /// This function initializes the Kubernetes client, loads the
    /// configuration, and dispatches to the appropriate subcommand's `run`
    /// method. It handles special cases for `Version` (client-only),
    /// `Completions`, and `DefaultConfig` output.
    ///
    /// # Returns
    ///
    /// A `Result` indicating the exit code (0 for success, non-zero for error)
    /// on success, or an `Error` if an unrecoverable issue occurs during
    /// execution.
    ///
    /// # Errors
    ///
    /// Returns an `Error` if:
    /// - The Kubernetes client cannot be initialized (e.g., `KubeConfigSnafu`).
    /// - The Tokio runtime fails to initialize (`InitializeTokioRuntimeSnafu`).
    /// - Any subcommand's `run` method returns an error.
    /// - Configuration loading fails via `load_config`.
    ///
    /// # Panics
    ///
    /// - This method `expect`s on `std::io::stdout().write_all()` operations.
    ///   In a typical CLI environment, writing to `stdout` or `stderr` is
    ///   expected to succeed.
    pub fn run(self) -> Result<i32, Error> {
        let client_version = Self::command().get_version().unwrap_or_default().to_string();
        match self.commands {
            Some(Commands::Version { client }) if client => {
                std::io::stdout()
                    .write_all(Self::command().render_long_version().as_bytes())
                    .expect("Failed to write to stdout");
                std::io::stdout()
                    .write_all(format!("Client Version: {client_version}\n").as_bytes())
                    .expect("Failed to write to stdout");

                return Ok(0);
            }
            Some(Commands::Completions { shell }) => {
                let mut app = Self::command();
                let bin_name = app.get_name().to_string();
                clap_complete::generate(shell, &mut app, bin_name, &mut std::io::stdout());
                return Ok(0);
            }
            Some(Commands::DefaultConfig) => {
                std::io::stdout()
                    .write_all(Config::template_basic().as_slice())
                    .expect("Failed to write to stdout");
                return Ok(0);
            }
            _ => {}
        }

        let config = self.load_config()?;
        config.log.registry();

        let fut = async move {
            let kube_client = kube::Client::try_default().await.context(error::KubeConfigSnafu)?;
            match self.commands {
                Some(Commands::Version { .. }) => {
                    let server_version = kube_client.apiserver_version().await.map_or_else(
                        |_| "unknown".to_string(),
                        |info| format!("{}.{}", info.major, info.minor),
                    );
                    let info = format!(
                        "Client Version: {client_version}\nServer Version: {server_version}\n",
                    );
                    std::io::stdout()
                        .write_all(Self::command().render_long_version().as_bytes())
                        .expect("Failed to write to stdout");
                    std::io::stdout()
                        .write_all(info.as_bytes())
                        .expect("Failed to write to stdout");

                    return Ok(0);
                }
                Some(Commands::Create(cmd)) => cmd.run(kube_client, config).boxed().await?,
                Some(Commands::List(cmd)) => cmd.run(kube_client, config).await?,
                Some(Commands::Attach(cmd)) => cmd.run(kube_client, config).await?,
                Some(Commands::Execute(cmd)) => cmd.run(kube_client, config).await?,
                Some(Commands::PortForward(cmd)) => cmd.run(kube_client, config).await?,
                Some(Commands::Delete(cmd)) => cmd.run(kube_client, config).await?,
                Some(Commands::Image { commands }) => commands.run(config).await?,
                Some(Commands::Ssh { commands }) => commands.run(kube_client, config).await?,
                _ => {
                    let help = Self::command().render_long_help().ansi().to_string();
                    std::io::stderr()
                        .write_all(help.as_bytes())
                        .expect("Failed to write to stdout");
                    return Ok(-1);
                }
            }

            Ok(0)
        };

        Runtime::new().context(error::InitializeTokioRuntimeSnafu)?.block_on(fut)
    }
}
