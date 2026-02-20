//! This module provides extensions to Kubernetes API types.
//!
//! It introduces traits and implementations that extend the functionality of
//! existing `k8s_openapi` types, such as `Pod`, with application-specific
//! methods for easier interaction and data extraction.

mod pod;

pub use self::pod::PodExt;
