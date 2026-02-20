//! This module provides extensions for `ObjectList<Pod>` to render a formatted
//! table.

use k8s_openapi::api::core::v1::Pod;
use kube::api::ObjectList;

/// Extension trait for `ObjectList<Pod>` to provide table rendering
/// capabilities.
pub trait PodListExt {
    /// Renders the list of pods into a human-readable table string.
    ///
    /// The table includes columns for "NAME", "IMAGE", "STATUS", "NAMESPACE",
    /// and "NODE".
    ///
    /// # Returns
    /// A `String` containing the formatted table.
    fn render_table(&self) -> String;
}

impl PodListExt for ObjectList<Pod> {
    /// Renders the list of pods into a human-readable table string.
    ///
    /// Each row in the table represents a pod, with columns for name, image,
    /// status, namespace, and node.
    ///
    /// # Returns
    /// A `String` containing the formatted table representation of the
    /// `ObjectList<Pod>`.
    ///
    /// # Example
    /// ```no_run
    /// use k8s_openapi::api::core::v1::Pod;
    /// use kube::api::{ObjectList, Meta, TypeMeta};
    /// use axon::ui::table::pod_list_ext::PodListExt; // Assuming `axon` is your crate name
    ///
    /// let pod_list = ObjectList {
    ///     metadata: Default::default(),
    ///     items: vec![
    ///         Pod {
    ///             metadata: Some(Meta {
    ///                 name: Some("my-pod-1".to_string()),
    ///                 namespace: Some("default".to_string()),
    ///                 ..Default::default()
    ///             }),
    ///             spec: Some(k8s_openapi::api::core::v1::PodSpec {
    ///                 containers: vec![
    ///                     k8s_openapi::api::core::v1::Container {
    ///                         image: Some("nginx:latest".to_string()),
    ///                         name: "nginx".to_string(),
    ///                         ..Default::default()
    ///                     },
    ///                 ],
    ///                 node_name: Some("worker-node-1".to_string()),
    ///                 ..Default::default()
    ///             }),
    ///             status: Some(k8s_openapi::api::core::v1::PodStatus {
    ///                 phase: Some("Running".to_string()),
    ///                 ..Default::default()
    ///             }),
    ///             ..Default::default()
    ///         },
    ///         Pod {
    ///             metadata: Some(Meta {
    ///                 name: Some("my-pod-2".to_string()),
    ///                 namespace: Some("kube-system".to_string()),
    ///                 ..Default::default()
    ///             }),
    ///             spec: Some(k8s_openapi::api::core::v1::PodSpec {
    ///                 containers: vec![
    ///                     k8s_openapi::api::core::v1::Container {
    ///                         image: Some("coredns:v1.8.0".to_string()),
    ///                         name: "coredns".to_string(),
    ///                         ..Default::default()
    ///                     },
    ///                 ],
    ///                 node_name: Some("worker-node-2".to_string()),
    ///                 ..Default::default()
    ///             }),
    ///             status: Some(k8s_openapi::api::core::v1::PodStatus {
    ///                 phase: Some("Pending".to_string()),
    ///                 ..Default::default()
    ///             }),
    ///             ..Default::default()
    ///         },
    ///     ],
    ///     ..Default::default()
    /// };
    ///
    /// let table_string = pod_list.render_table();
    /// println!("{}", table_string);
    /// ```
    fn render_table(&self) -> String {
        let rows = self.items.iter().map(pod_column).collect::<Vec<_>>();
        comfy_table::Table::new()
            .load_preset(comfy_table::presets::NOTHING)
            .set_content_arrangement(comfy_table::ContentArrangement::Dynamic)
            .set_header(vec!["NAME", "IMAGE", "STATUS", "NAMESPACE", "NODE"])
            .add_rows(rows)
            .to_string()
    }
}

/// Extracts specific column data for a single Kubernetes `Pod` object.
///
/// This function retrieves the pod's name, the image of its first container,
/// its status phase, namespace, and the node it's scheduled on.
/// Defaults are used if any information is missing.
///
/// # Arguments
/// * `pod` - A reference to the `Pod` object from which to extract data.
///
/// # Returns
/// An array of five `String`s, representing the column values in the order:
/// `[NAME, IMAGE, STATUS, NAMESPACE, NODE]`.
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
