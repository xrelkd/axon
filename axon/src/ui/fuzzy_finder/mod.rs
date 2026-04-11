//! This module provides utilities for fuzzy finding Kubernetes pods using the
//! `skim` library, including a common column separator and re-exports for
//! extended pod list functionality.

mod pod_list;

/// The default column separator used for formatting output in UI tables.
///
/// This constant defines the string used to separate columns when displaying
/// data in the console or other text-based UI components. It is typically a
/// tab character to allow for easy parsing or alignment.
pub const COLUMN_SEPARATOR: &str = "\t";

/// Re-exports the `PodListExt` trait from the `pod_list` submodule.
///
/// This trait provides extended functionality for collections of Kubernetes
/// pods, particularly for fuzzy finding and selecting pods using `skim`.
pub use self::pod_list::PodListExt;
