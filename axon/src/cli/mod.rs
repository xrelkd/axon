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
use crate::{config::Config, shadow};

#[derive(Parser)]
#[command(
    name = axon_base::CLI_PROGRAM_NAME,
    author,
    version,
    long_version = shadow::CLAP_LONG_VERSION,
    about = "Axon CLI: A robust tool for advanced Kubernetes resource management.",
    long_about = "Axon is a powerful Rust-based Command Line Interface (CLI) tool designed for advanced interaction with Kubernetes resources. It provides extended functionality and a specialized interface for common Kubernetes operational tasks, including pod management, image handling, and secure shell access.",
    color = clap::ColorChoice::Always
)]
#[command()]
pub struct Cli {
    #[clap(subcommand)]
    commands: Option<Commands>,

    #[clap(
        long = "config",
        short = 'c',
        env = "AXON_CONFIG_FILE_PATH",
        help = "Specify a configuration file. Defaults to ~/.config/axon/config.yaml or \
                AXON_CONFIG_FILE_PATH env var."
    )]
    config_file: Option<PathBuf>,

    #[clap(
        long = "log-level",
        env = "AXON_LOG_LEVEL",
        help = "Set the logging level (e.g., info, debug, trace)."
    )]
    log_level: Option<tracing::Level>,
}

#[derive(Clone, Subcommand)]
pub enum Commands {
    #[command(about = "Display client and server version information")]
    Version {
        #[clap(long = "client", help = "If true, shows client version only (no server required).")]
        client: bool,
    },

    #[command(about = "Generate shell completion script for the specified shell (bash, zsh, fish)")]
    Completions { shell: clap_complete::Shell },

    #[command(about = "Output the default configuration in YAML format")]
    DefaultConfig,

    #[command(
        alias = "c",
        about = "Create a new temporary pod in a specified namespace or using a predefined spec"
    )]
    Create(CreateCommand),

    #[command(alias = "d", about = "Delete one or more temporary pods managed by Axon")]
    Delete(DeleteCommand),

    #[command(alias = "a", about = "Attach to a running temporary pod's console")]
    Attach(AttachCommand),

    #[command(
        aliases = ["e", "exec"],
        about = "Execute a command inside a running temporary pod"
    )]
    Execute(ExecuteCommand),

    #[command(alias = "l", about = "List all temporary pods managed by Axon")]
    List(ListCommand),

    #[command(
        aliases = ["p", "pf"],
        about = "Forward one or more local ports to a specific port on a temporary pod"
    )]
    PortForward(PortForwardCommand),

    #[command(alias = "i", about = "Manage container image specifications")]
    Image {
        #[command(subcommand)]
        commands: ImageCommands,
    },

    #[command(
        about = "Securely interact with a temporary pod via SSH (shell, file transfer, setup)"
    )]
    Ssh {
        #[command(subcommand)]
        commands: SshCommands,
    },
}

impl Default for Cli {
    fn default() -> Self { Self::parse() }
}

impl Cli {
    fn load_config(&self) -> Result<Config, Error> {
        let mut config =
            Config::load(self.config_file.clone().unwrap_or_else(Config::search_config_file_path))?;

        if let Some(log_level) = self.log_level {
            config.log.level = log_level;
        }

        Ok(config)
    }

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
                let config_text =
                    serde_yaml::to_string(&Config::default()).expect("Config is serializable");
                std::io::stdout()
                    .write_all(config_text.as_bytes())
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
                Some(Commands::Delete(cmd)) => cmd.run(kube_client).await?,
                Some(Commands::List(cmd)) => cmd.run(kube_client).await?,
                Some(Commands::Attach(cmd)) => cmd.run(kube_client, config).await?,
                Some(Commands::Execute(cmd)) => cmd.run(kube_client, config).await?,
                Some(Commands::PortForward(cmd)) => cmd.run(kube_client, config).await?,
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
