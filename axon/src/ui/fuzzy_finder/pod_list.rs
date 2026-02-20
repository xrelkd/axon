//! This module provides extensions for working with Kubernetes `Pod` objects,
//! specifically for integrating them with the `skim` fuzzy finder library.
//! It allows for displaying `Pod` information in a user-friendly format within
//! `skim` and for selecting pods from a list.

use std::{borrow::Cow, sync::Arc};

use k8s_openapi::api::core::v1::Pod;
use kube::api::ObjectList;
use skim::{
    Skim, SkimItem, SkimItemReceiver, SkimItemSender, SkimOptions,
    prelude::{SkimOptionsBuilder, unbounded},
};

use crate::ui::fuzzy_finder::COLUMN_SEPARATOR;

/// Extension trait for `ObjectList<Pod>` to facilitate fuzzy finding and
/// selection of pods.
pub trait PodListExt {
    /// Converts a list of Kubernetes `Pod` objects into a vector of `Arc<dyn
    /// SkimItem>` suitable for use with the `skim` fuzzy finder.
    ///
    /// This method is primarily used internally to prepare data for the fuzzy
    /// finder.
    ///
    /// # Returns
    /// A `Vec` of `Arc<dyn SkimItem>` where each item represents a Kubernetes
    /// Pod.
    fn items(&self) -> Vec<Arc<dyn SkimItem>>;

    /// Displays a fuzzy finder interface to the user, allowing them to select
    /// one or more `Pod` names from the list.
    ///
    /// If no items are available, an empty vector is returned immediately.
    ///
    /// # Panics
    /// This method panics if the `tokio::task::spawn_blocking` task fails to
    /// join, which should ideally not happen under normal circumstances.
    ///
    /// # Returns
    /// A `Vec<String>` containing the names of the selected pods. If the user
    /// aborts the skim interface or no pods are selected, an empty vector
    /// is returned.
    ///
    /// # Example
    /// ```no_run
    /// use k8s_openapi::api::core::v1::Pod;
    /// use kube::api::{ObjectList, Meta};
    /// use std::collections::BTreeMap;
    /// use std::sync::Arc;
    /// use axon::ui::fuzzy_finder::pod_list::PodListExt;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     // Simulate an ObjectList<Pod>
    ///     let mut pod_metadata = k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta::default();
    ///     pod_metadata.name = Some("my-pod-1".to_string());
    ///     pod_metadata.namespace = Some("default".to_string());
    ///     let pod1 = Pod { metadata: pod_metadata.clone(), ..Default::default() };
    ///
    ///     pod_metadata.name = Some("my-pod-2".to_string());
    ///     let pod2 = Pod { metadata: pod_metadata, ..Default::default() };
    ///
    ///     let pod_list = ObjectList {
    ///         items: vec![pod1, pod2],
    ///         ..Default::default()
    ///     };
    ///
    ///     let selected_pod_names = pod_list.find_pod_names().await;
    ///     println!("Selected pods: {:?}", selected_pod_names);
    ///     Ok(())
    /// }
    /// ```
    async fn find_pod_names(&self) -> Vec<String> {
        let items = self.items();
        if items.is_empty() {
            return Vec::new();
        }

        tokio::task::spawn_blocking(move || {
            let (tx_item, rx_item): (SkimItemSender, SkimItemReceiver) = unbounded();
            for item in items {
                drop(tx_item.send(item));
            }
            drop(tx_item);

            let options = generate_skim_options();
            if let Some(out) = Skim::run_with(&options, Some(rx_item)) {
                if out.is_abort {
                    return Vec::new();
                }
                out.selected_items.iter().map(|item| item.output().to_string()).collect()
            } else {
                Vec::new()
            }
        })
        .await
        .expect("Failed to join spawn_blocking task")
    }
}

/// Implements `PodListExt` for `kube::api::ObjectList<Pod>`, allowing direct
/// use of the fuzzy finding capabilities on lists of Kubernetes Pods.
impl PodListExt for ObjectList<Pod> {
    fn items(&self) -> Vec<Arc<dyn SkimItem>> {
        self.iter()
            .map(|pod| -> Arc<dyn SkimItem> { Arc::new(PodSkimItem::from(pod.clone())) })
            .collect()
    }
}

/// A wrapper struct for `k8s_openapi::api::core::v1::Pod` that implements the
/// `SkimItem` trait, making `Pod` objects compatible with the `skim` fuzzy
/// finder.
pub struct PodSkimItem(Pod);

/// Implements the `From` trait to convert a `k8s_openapi::api::core::v1::Pod`
/// into a `PodSkimItem`.
impl From<Pod> for PodSkimItem {
    fn from(value: Pod) -> Self { Self(value) }
}

/// Implements the `SkimItem` trait for `PodSkimItem`, defining how a `Pod` is
/// displayed and interacted with within the `skim` fuzzy finder.
impl SkimItem for PodSkimItem {
    /// Returns the primary text used by `skim` for matching and display.
    /// This is typically the pod's name.
    ///
    /// # Returns
    /// A `Cow<'_, str>` representing the pod's name, or an empty string if the
    /// name is not set.
    fn text(&self) -> Cow<'_, str> { self.0.metadata.name.clone().unwrap_or_default().into() }

    /// Returns the output string when the item is selected.
    /// This is typically the pod's name, used for retrieving the selected
    /// item's identifier.
    ///
    /// # Returns
    /// A `Cow<'_, str>` representing the pod's name, or an empty string if the
    /// name is not set.
    fn output(&self) -> Cow<'_, str> { self.0.metadata.name.clone().unwrap_or_default().into() }

    /// Defines how the `PodSkimItem` is displayed in the `skim` interface,
    /// arranging pod information into columns.
    ///
    /// # Arguments
    /// * `_context` - The display context provided by `skim`, currently unused.
    ///
    /// # Returns
    /// An `AnsiString` representing the formatted pod information with ANSI
    /// escape codes for potential coloring or styling.
    fn display<'a>(&'a self, _context: skim::DisplayContext<'a>) -> skim::AnsiString<'a> {
        skim::AnsiString::from(pod_column(&self.0).join(COLUMN_SEPARATOR))
    }
}

/// Extracts key information from a Kubernetes `Pod` object and formats it into
/// an array of strings, suitable for displaying in a tabular format within the
/// `skim` fuzzy finder.
///
/// The columns extracted are: Name, Image, Phase, Namespace, and Node Name.
/// Default values are used if specific fields are not available.
///
/// # Arguments
/// * `pod` - A reference to the `Pod` object from which to extract information.
///
/// # Returns
/// An array `[String; 5]` containing the formatted strings for each column.
fn pod_column(pod: &Pod) -> [String; 5] {
    [
        pod.metadata.name.clone().unwrap_or_default(),
        pod.spec
            .as_ref()
            .and_then(|s| s.containers.first())
            .map(|c| c.image.clone().unwrap_or_default())
            .unwrap_or_default(),
        pod.status.as_ref().and_then(|s| s.phase.clone()).unwrap_or_else(|| "Unknown".to_string()),
        pod.metadata.namespace.clone().unwrap_or_default(),
        pod.spec.as_ref().and_then(|s| s.node_name.clone()).unwrap_or_default(),
    ]
}

/// Generates the default `SkimOptions` used for the pod fuzzy finder.
///
/// Currently, it configures the fuzzy finder to take up 100% of the terminal
/// height and allows only single item selection.
///
/// # Panics
/// This function panics if the `SkimOptionsBuilder` fails to build the options,
/// which indicates a configuration error in the `skim` library usage.
///
/// # Returns
/// A `SkimOptions` struct configured for pod selection.
fn generate_skim_options() -> SkimOptions {
    SkimOptionsBuilder::default()
        .height("100%".to_string())
        .multi(false)
        .build()
        .expect("Skim options build failed")
}
