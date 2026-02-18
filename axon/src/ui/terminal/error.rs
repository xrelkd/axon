use snafu::Snafu;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    #[snafu(display("Failed to enable terminal raw mode, error: {source}"))]
    EnableTerminalRawMode { source: std::io::Error },
}
