//! Configuration and initialization for application logging.
//!
//! This module provides the `LogConfig` struct for defining logging
//! preferences, such as output targets (stdout, stderr, journald, file) and log
//! level. It also includes the `LogDriver` enum and associated logic for
//! creating `tracing` layers based on the configured `LogConfig`.
use std::{fs::OpenOptions, path::PathBuf};

use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, serde_as};
use tracing_subscriber::{
    Layer, layer::SubscriberExt, registry::LookupSpan, util::SubscriberInitExt,
};

/// Represents the configuration for the application's logging system.
///
/// This struct allows specifying where log messages should be emitted (e.g.,
/// stdout, stderr, journald, or a file) and at what level (e.g., INFO, DEBUG).
/// It integrates with `serde` for easy serialization and deserialization from
/// configuration sources.
#[serde_as]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LogConfig {
    /// Optional path to a file where logs should be written.
    /// If `None`, logs will not be written to a file.
    #[serde(default = "LogConfig::default_file_path")]
    pub file_path: Option<PathBuf>,

    /// A boolean indicating whether logs should be emitted to `journald`.
    #[serde(default = "LogConfig::default_emit_journald")]
    pub emit_journald: bool,

    /// A boolean indicating whether logs should be emitted to standard output.
    #[serde(default = "LogConfig::default_emit_stdout")]
    pub emit_stdout: bool,

    /// A boolean indicating whether logs should be emitted to standard error.
    #[serde(default = "LogConfig::default_emit_stderr")]
    pub emit_stderr: bool,

    /// The minimum log level to be recorded.
    /// Messages with a level below this will be filtered out.
    #[serde(default = "LogConfig::default_log_level")]
    #[serde_as(as = "DisplayFromStr")]
    pub level: tracing::Level,
}

impl Default for LogConfig {
    /// Returns a default `LogConfig` with common settings.
    ///
    /// By default, logs are set to `INFO` level, emitted to `journald` and
    /// `stdout`, but not `stderr` or a file.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tracing::Level;
    /// use axon_config::log::LogConfig; // Assuming 'axon_config' is the crate name
    ///
    /// let default_config = LogConfig::default();
    /// assert_eq!(default_config.level, Level::INFO);
    /// assert!(default_config.emit_journald);
    /// assert!(default_config.emit_stdout);
    /// assert!(!default_config.emit_stderr);
    /// assert!(default_config.file_path.is_none());
    /// ```
    fn default() -> Self {
        Self {
            file_path: Self::default_file_path(),
            emit_journald: Self::default_emit_journald(),
            emit_stdout: Self::default_emit_stdout(),
            emit_stderr: Self::default_emit_stderr(),
            level: Self::default_log_level(),
        }
    }
}

impl LogConfig {
    /// Returns the default log level, which is `INFO`.
    #[inline]
    #[must_use]
    pub const fn default_log_level() -> tracing::Level { tracing::Level::INFO }

    /// Returns the default file path for logs, which is `None`.
    #[inline]
    #[must_use]
    pub const fn default_file_path() -> Option<PathBuf> { None }

    /// Returns the default setting for `emit_journald`, which is `true`.
    #[inline]
    #[must_use]
    pub const fn default_emit_journald() -> bool { true }

    /// Returns the default setting for `emit_stdout`, which is `true`.
    #[inline]
    #[must_use]
    pub const fn default_emit_stdout() -> bool { true }

    /// Returns the default setting for `emit_stderr`, which is `false`.
    #[inline]
    #[must_use]
    pub const fn default_emit_stderr() -> bool { false }

    /// Initializes the global `tracing` subscriber registry based on this
    /// `LogConfig`.
    ///
    /// This method sets up the logging infrastructure, directing logs to the
    /// specified outputs (journald, file, stdout, stderr) and applying the
    /// configured log level.
    ///
    /// # Panics
    ///
    /// This method panics if called more than once in the same application
    /// lifetime, as `tracing_subscriber::util::SubscriberInitExt::init()`
    /// will panic if a global subscriber is already set.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use tracing::Level;
    /// use axon::log::LogConfig; // Assuming 'axon_config' is the crate name
    ///
    /// // Create a configuration to log INFO level messages to stdout.
    /// let config = LogConfig {
    ///     level: Level::INFO,
    ///     emit_stdout: true,
    ///     ..Default::default()
    /// };
    ///
    /// // Initialize the logger.
    /// // Note: In a real application, you'd typically call this once at startup.
    /// // For testing, you might need to ensure no other subscriber is initialized.
    /// // config.registry();
    ///
    /// // Now you can use tracing macros:
    /// // tracing::info!("This is an info message.");
    /// // tracing::debug!("This debug message might not appear depending on the level.");
    /// ```
    pub fn registry(&self) {
        let Self { emit_journald, file_path, emit_stdout, emit_stderr, level: log_level } = self;

        let filter_layer = tracing_subscriber::filter::LevelFilter::from_level(*log_level);

        tracing_subscriber::registry()
            .with(filter_layer)
            .with(emit_journald.then(|| LogDriver::Journald.layer()))
            .with(file_path.clone().map(|path| LogDriver::File(path).layer()))
            .with(emit_stdout.then(|| LogDriver::Stdout.layer()))
            .with(emit_stderr.then(|| LogDriver::Stderr.layer()))
            .init();
    }
}

/// Enumerates the possible log output drivers.
///
/// This enum represents the various destinations where log messages can be
/// sent.
#[derive(Clone, Debug)]
enum LogDriver {
    /// Logs will be written to standard output.
    Stdout,
    /// Logs will be written to standard error.
    Stderr,
    /// Logs will be written to the system's `journald` service.
    Journald,
    /// Logs will be written to a specified file path.
    File(PathBuf),
}

impl LogDriver {
    /// Creates a `tracing_subscriber::Layer` for the specific log driver.
    ///
    /// This method configures a `tracing` layer that directs formatted log
    /// messages to the output specified by the `LogDriver` variant.
    ///
    /// # Type Parameters
    ///
    /// * `S`: The `tracing::Subscriber` type that this layer will be attached
    ///   to.
    ///
    /// # Returns
    ///
    /// An `Option` containing a `Box<dyn Layer<S> + Send + Sync + 'static>` if
    /// the layer could be successfully created, or `None` if there was an
    /// error (e.g., failing to open a log file or initialize `journald`).
    ///
    /// # Errors
    ///
    /// Returns `None` if:
    /// - For `LogDriver::File`, the specified file cannot be opened for
    ///   appending or creation.
    /// - For `LogDriver::Journald`, `tracing_journald::layer()` fails to
    ///   initialize the layer.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::path::PathBuf;
    /// use tracing_subscriber::{
    ///     Layer, layer::SubscriberExt, registry::LookupSpan, util::SubscriberInitExt,
    /// };
    /// use axon_config::log::{LogDriver, LogConfig}; // Assuming 'axon_config' is the crate name
    ///
    /// // Example of creating a layer for stdout:
    /// let stdout_layer = LogDriver::Stdout.layer();
    /// assert!(stdout_layer.is_some());
    ///
    /// // Example of creating a layer for a file (might fail if path is invalid or permissions issue):
    /// let file_path = PathBuf::from("/tmp/my_app_test.log");
    /// let file_layer = LogDriver::File(file_path.clone()).layer();
    /// // In a real scenario, you would check file_layer.is_some() and handle potential errors.
    ///
    /// // You can then use these layers to initialize a subscriber:
    /// // tracing_subscriber::registry()
    /// //     .with(stdout_layer)
    /// //     .init();
    /// // tracing::info!("Logs are now going to stdout!");
    /// std::fs::remove_file(file_path).ok(); // Clean up test file
    /// ```
    #[allow(clippy::type_repetition_in_bounds)]
    fn layer<S>(self) -> Option<Box<dyn Layer<S> + Send + Sync + 'static>>
    where
        S: tracing::Subscriber,
        for<'a> S: LookupSpan<'a>,
    {
        // Shared configuration regardless of where logs are output to.
        let fmt =
            tracing_subscriber::fmt::layer().pretty().with_thread_ids(true).with_thread_names(true);

        // Configure the writer based on the desired log target:
        match self {
            Self::Stdout => Some(Box::new(fmt.with_writer(std::io::stdout))),
            Self::Stderr => Some(Box::new(fmt.with_writer(std::io::stderr))),
            Self::File(path) => {
                let file = OpenOptions::new().create(true).append(true).open(path).ok()?;
                Some(Box::new(fmt.with_writer(file)))
            }
            Self::Journald => Some(Box::new(tracing_journald::layer().ok()?)),
        }
    }
}
