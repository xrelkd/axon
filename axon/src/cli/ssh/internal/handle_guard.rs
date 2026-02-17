use crate::cli::Error;

pub struct HandleGuard {
    handle: sigfinn::Handle<Error>,
}

impl From<sigfinn::Handle<Error>> for HandleGuard {
    fn from(handle: sigfinn::Handle<Error>) -> Self { Self { handle } }
}

impl Drop for HandleGuard {
    fn drop(&mut self) { self.handle.shutdown(); }
}
