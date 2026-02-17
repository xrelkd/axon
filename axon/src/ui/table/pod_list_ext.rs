use k8s_openapi::api::core::v1::Pod;
use kube::api::ObjectList;

pub trait PodListExt {
    fn render_table(&self) -> String;
}

impl PodListExt for ObjectList<Pod> {
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
