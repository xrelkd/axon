//! This module provides utilities for displaying tabular data in the user
//! interface, including a common column separator and re-exports for displaying
//! extended pod list functionality.

mod pod_list;

/// The default column separator used for formatting output in UI tables.
///
/// This constant defines the string used to separate columns when displaying
/// tabular data in the console or other text-based UI components. It is
/// typically a tab character to allow for easy parsing or alignment.
///
/// # Examples
///
/// ```rust
/// use crate::ui::table::COLUMN_SEPARATOR;
///
/// let column1 = "Name";
/// let column2 = "Status";
/// let column3 = "ID";
///
/// let row = format!("{}{}{}{}{}", column1, COLUMN_SEPARATOR, column2, COLUMN_SEPARATOR, column3);
/// assert_eq!(row, "Name\tStatus\tID");
/// ```
pub const COLUMN_SEPARATOR: &str = "\t";

/// Re-exports the `PodListExt` trait from the `pod_list` submodule.
///
/// This trait is intended to provide extended functionality for collections
/// or lists of Kubernetes pods, particularly for formatting and displaying
/// pod-related information in UI tables within this module's context.
///
/// Types implementing this trait can leverage shared logic for presenting
/// pod data consistently.
pub use self::pod_list::PodListExt;
