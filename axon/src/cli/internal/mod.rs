mod api_pod;
mod resource;

pub use self::{
    api_pod::ApiPodExt,
    resource::{ResolvedResources, ResourceResolver},
};
