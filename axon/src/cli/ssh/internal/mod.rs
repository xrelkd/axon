mod configurator;
mod handle_guard;

pub use self::{configurator::SshConfigurator, handle_guard::HandleGuard};

pub const DEFAULT_SSH_PORT: u16 = 22;
