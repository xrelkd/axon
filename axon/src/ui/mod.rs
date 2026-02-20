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
///
/// # Examples
///
/// ```rust
/// use axon::ui::FileTransferProgressBar;
/// use std::thread;
/// use std::time::Duration;
///
/// // Create a new progress bar with a total size of 100 units
/// let progress_bar = FileTransferProgressBar::new("Downloading file", 100);
///
/// // Simulate progress
/// for i in 0..=100 {
///     progress_bar.set_progress(i);
///     thread::sleep(Duration::from_millis(10));
/// }
///
/// progress_bar.finish();
/// ```
pub use self::file_transfer_progress_bar::FileTransferProgressBar;
