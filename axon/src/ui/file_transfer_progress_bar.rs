use tokio::io::AsyncRead;

pub struct FileTransferProgressBar {
    inner: indicatif::ProgressBar,
    direction: Direction,
}

impl FileTransferProgressBar {
    pub fn new_upload() -> Self { Self::new(Direction::Upload) }

    pub fn new_download() -> Self { Self::new(Direction::Download) }

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
                .unwrap()
                .progress_chars("#>-"),
        );
        inner.set_message(msg);
        Self { inner, direction }
    }

    pub fn set_length(&self, len: u64) { self.inner.set_length(len); }

    pub fn wrap_async_read<R: AsyncRead + Unpin>(&self, read: R) -> impl AsyncRead + Unpin {
        self.inner.wrap_async_read(read)
    }

    pub fn finish(self) {
        let msg = match self.direction {
            Direction::Upload => "Upload completed",
            Direction::Download => "Download completed",
        };
        self.inner.finish_with_message(msg);
    }
}

#[derive(Clone, Copy, Debug)]
enum Direction {
    Download,
    Upload,
}
