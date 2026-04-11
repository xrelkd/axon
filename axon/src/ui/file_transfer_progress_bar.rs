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
    pub fn new_upload() -> Self { Self::new(Direction::Upload) }

    /// Creates a new `FileTransferProgressBar` configured for a download
    /// operation.
    ///
    /// The progress bar will display "Downloading" as its message.
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
    pub fn wrap_async_read<R: AsyncRead + Unpin>(&self, read: R) -> impl AsyncRead + Unpin {
        self.inner.wrap_async_read(read)
    }

    /// Finishes the progress bar, setting its message to indicate completion
    /// (e.g., "Upload completed" or "Download completed").
    ///
    /// This consumes the `FileTransferProgressBar` instance.
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
