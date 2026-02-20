//! This module provides extensions for Kubernetes Pod lists and specifications.
//!
//! It re-exports `PodListExt` and `SpecExt` traits, which offer additional
//! functionality and helper methods for working with Kubernetes Pod data
//! structures.

mod pod_list_ext;
mod spec_ext;

/// Re-exports the [`PodListExt`] trait, which provides extension methods for
/// lists of Kubernetes Pods.
///
/// This trait is intended to add convenience methods to `Vec<Pod>` or similar
/// collections for common operations like filtering, sorting, or extracting
/// information.
pub use self::{pod_list_ext::PodListExt, spec_ext::SpecExt};
