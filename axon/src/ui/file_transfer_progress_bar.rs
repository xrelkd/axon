//! Provides a progress bar for file transfers, indicating upload or download
//! progress.

use tokio::io::AsyncRead;

/// A progress bar specifically designed for file transfer operations,
/// indicating either an upload or a download.
pub struct FileTransferProgressBar {
    /// The inner `indicatif::ProgressBar` instance that manages the progress
    /// display.
    inner: indicatif::ProgressBar,
    /// The direction of the file transfer (Upload or Download).
    direction: Direction,
}

impl FileTransferProgressBar {
    /// Creates a new `FileTransferProgressBar` configured for an upload
    /// operation.
    ///
    /// The progress bar will display "Uploading" as its message.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use axon::ui::file_transfer_progress_bar::FileTransferProgressBar;
    ///
    /// let upload_bar = FileTransferProgressBar::new_upload();
    /// upload_bar.set_length(100);
    /// // ... use upload_bar.wrap_async_read(...)
    /// upload_bar.finish();
    /// ```
    pub fn new_upload() -> Self { Self::new(Direction::Upload) }

    /// Creates a new `FileTransferProgressBar` configured for a download
    /// operation.
    ///
    /// The progress bar will display "Downloading" as its message.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use axon::ui::file_transfer_progress_bar::FileTransferProgressBar;
    ///
    /// let download_bar = FileTransferProgressBar::new_download();
    /// download_bar.set_length(200);
    /// // ... use download_bar.wrap_async_read(...)
    /// download_bar.finish();
    /// ```
    pub fn new_download() -> Self { Self::new(Direction::Download) }

    /// Creates a new `FileTransferProgressBar` with a specified transfer
    /// direction.
    ///
    /// This private constructor initializes the `indicatif::ProgressBar` with a
    /// default style and sets the appropriate message ("Uploading" or
    /// "Downloading").
    ///
    /// # Arguments
    ///
    /// * `direction` - The `Direction` of the file transfer (Upload or
    ///   Download).
    ///
    /// # Panics
    ///
    /// This function will panic if the progress bar template string is invalid.
    /// However, with a hardcoded valid template, this should not occur.
    fn new(direction: Direction) -> Self {
        let msg = match direction {
            Direction::Upload => "Uploading",
            Direction::Download => "Downloading",
        };
        let inner = indicatif::ProgressBar::new(0);
        inner.set_style(
            indicatif::ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] \
                     {bytes}/{total_bytes} ({eta}) {msg}",
                )
                .expect("the template is valid")
                .progress_chars("#>-"),
        );
        inner.set_message(msg);
        Self { inner, direction }
    }

    /// Sets the total length of the progress bar, typically representing the
    /// total bytes to be transferred.
    ///
    /// # Arguments
    ///
    /// * `len` - The total number of units (e.g., bytes) for the progress bar.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use axon::ui::file_transfer_progress_bar::FileTransferProgressBar;
    ///
    /// let bar = FileTransferProgressBar::new_upload();
    /// bar.set_length(1024 * 1024); // Set total to 1MB
    /// ```
    pub fn set_length(&self, len: u64) { self.inner.set_length(len); }

    /// Wraps an `AsyncRead` implementer with the progress bar, allowing it to
    /// track the progress of the read operation.
    ///
    /// # Type Parameters
    ///
    /// * `R` - A type that implements `tokio::io::AsyncRead` and `Unpin`.
    ///
    /// # Arguments
    ///
    /// * `read` - The asynchronous reader to wrap.
    ///
    /// # Returns
    ///
    /// An implementer of `tokio::io::AsyncRead` and `Unpin` that will update
    /// the progress bar as bytes are read.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use axon::ui::file_transfer_progress_bar::FileTransferProgressBar;
    /// use tokio::io::{AsyncReadExt, Result};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<()> {
    ///     let data = b"Hello, world!";
    ///     let cursor = tokio::io::Cursor::new(data);
    ///     let bar = FileTransferProgressBar::new_upload();
    ///     bar.set_length(data.len() as u64);
    ///
    ///     let mut reader_with_progress = bar.wrap_async_read(cursor);
    ///     let mut buffer = Vec::new();
    ///     reader_with_progress.read_to_end(&mut buffer).await?;
    ///
    ///     bar.finish();
    ///     Ok(())
    /// }
    /// ```
    pub fn wrap_async_read<R: AsyncRead + Unpin>(&self, read: R) -> impl AsyncRead + Unpin {
        self.inner.wrap_async_read(read)
    }

    /// Finishes the progress bar, setting its message to indicate completion
    /// (e.g., "Upload completed" or "Download completed").
    ///
    /// This consumes the `FileTransferProgressBar` instance.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use axon::ui::file_transfer_progress_bar::FileTransferProgressBar;
    ///
    /// let bar = FileTransferProgressBar::new_download();
    /// bar.set_length(500);
    /// // Simulate some progress
    /// bar.inner.inc(200);
    /// bar.finish();
    /// ```
    pub fn finish(self) {
        let msg = match self.direction {
            Direction::Upload => "Upload completed",
            Direction::Download => "Download completed",
        };
        self.inner.finish_with_message(msg);
    }
}

/// Represents the direction of a file transfer operation.
#[derive(Clone, Copy, Debug)]
enum Direction {
    /// Indicates that the file is being downloaded.
    Download,
    /// Indicates that the file is being uploaded.
    Upload,
}
