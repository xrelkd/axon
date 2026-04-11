//! Contains various UI components used throughout the application.
//!
//! This module re-exports several UI-related components, including:
//! - [`file_transfer_progress_bar`]: For displaying progress during file
//!   transfers.
//! - [`fuzzy_finder`]: For interactive, fuzzy searching of items.
//! - [`table`]: For displaying data in a tabular format.
//! - [`terminal`]: For terminal-specific UI functionalities.

mod file_transfer_progress_bar;
pub mod fuzzy_finder;
pub mod table;
pub mod terminal;

/// Re-exports the [`FileTransferProgressBar`] struct for displaying file
/// transfer progress.
///
/// This struct provides functionality to create and update a progress bar,
/// typically used in a terminal UI, to visualize the progress of file upload or
/// download operations.
pub use self::file_transfer_progress_bar::FileTransferProgressBar;
