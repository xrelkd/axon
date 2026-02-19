use std::{borrow::Cow, sync::Arc};

use k8s_openapi::api::core::v1::Pod;
use kube::api::ObjectList;
use skim::{
    Skim, SkimItem, SkimItemReceiver, SkimItemSender,
    prelude::{SkimOptionsBuilder, unbounded},
};

use crate::ui::fuzzy_finder::COLUMN_SEPARATOR;

pub trait PodListExt {
    fn items(&self) -> Vec<Arc<dyn SkimItem>>;

    async fn select_pod_names(&self) -> Vec<String> {
        if self.items().is_empty() {
            return Vec::new();
        }

        let (tx_item, rx_item): (SkimItemSender, SkimItemReceiver) = unbounded();
        for item in self.items() {
            drop(tx_item.send(item));
        }
        drop(tx_item);

        let options = SkimOptionsBuilder::default()
            .height("100%".to_string())
            .multi(false)
            .build()
            .expect("Skim options build failed");

        if let Some(out) = Skim::run_with(&options, Some(rx_item)) {
            if out.is_abort {
                return Vec::new();
            }
            out.selected_items.iter().map(|item| item.output().to_string()).collect()
        } else {
            Vec::new()
        }
    }
}

impl PodListExt for ObjectList<Pod> {
    fn items(&self) -> Vec<Arc<dyn SkimItem>> {
        self.iter()
            .map(|pod| Arc::new(PodSkimItem::from(pod.clone())) as Arc<dyn SkimItem>)
            .collect()
    }
}

pub struct PodSkimItem(Pod);

impl From<Pod> for PodSkimItem {
    fn from(value: Pod) -> Self { Self(value) }
}

impl SkimItem for PodSkimItem {
    fn text(&self) -> Cow<'_, str> { self.0.metadata.name.clone().unwrap_or_default().into() }

    fn output(&self) -> Cow<'_, str> { self.0.metadata.name.clone().unwrap_or_default().into() }

    fn display<'a>(&'a self, _context: skim::DisplayContext<'a>) -> skim::AnsiString<'a> {
        skim::AnsiString::from(pod_column(&self.0).join(COLUMN_SEPARATOR))
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
