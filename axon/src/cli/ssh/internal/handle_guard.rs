use crate::cli::Error;

/// A guard that manages the shutdown of a `sigfinn` handle.
///
/// This struct holds a `sigfinn::Handle<Error>` and ensures that the
/// associated background task or resource is cleanly shut down when
/// the `HandleGuard` goes out of scope. It leverages Rust's `Drop`
/// trait for automatic resource management.
pub struct HandleGuard {
    /// The `sigfinn` handle that this guard is responsible for shutting down.
    handle: sigfinn::Handle<Error>,
}

impl From<sigfinn::Handle<Error>> for HandleGuard {
    /// Creates a new `HandleGuard` from a `sigfinn::Handle<Error>`.
    ///
    /// This conversion allows for easy wrapping of a `sigfinn` handle
    /// into a `HandleGuard` for automatic shutdown management.
    ///
    /// # Arguments
    ///
    /// * `handle` - The `sigfinn::Handle<Error>` to be managed by this guard.
    ///
    /// # Returns
    ///
    /// A new `HandleGuard` instance.
    fn from(handle: sigfinn::Handle<Error>) -> Self { Self { handle } }
}

impl Drop for HandleGuard {
    /// Shuts down the `sigfinn` handle when the `HandleGuard` is dropped.
    ///
    /// This implementation ensures that the background task or resource
    /// associated with the `sigfinn::Handle` is gracefully terminated
    /// when the `HandleGuard` instance goes out of scope.
    fn drop(&mut self) { self.handle.shutdown(); }
}
