//! Internal utilities and extensions for CLI commands.
//!
//! This module provides foundational traits and structures used internally by
//! various CLI commands to interact with the Axon API and resolve resources.
//!
//! It re-exports key components from its sub-modules, `api_pod` and `resource`,
//! to facilitate their use across the CLI.

mod api_pod;
mod resource;

pub use self::{
    api_pod::ApiPodExt,
    resource::{ResolvedResources, ResourceResolver},
};
