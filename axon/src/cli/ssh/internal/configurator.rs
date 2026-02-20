//! This module defines the `Configurator` struct, which provides functionality
//! for interacting with Kubernetes pods, specifically for managing SSH keys.

use std::fmt;

use k8s_openapi::api::core::v1::Pod;
use kube::{Api, api::AttachParams};
use snafu::ResultExt;

use crate::cli::{Error, error};

/// Manages configuration tasks for a specific Kubernetes pod, such as uploading
/// SSH keys.
pub struct Configurator {
    /// The Kubernetes API client for interacting with Pod resources.
    api: Api<Pod>,
    /// The namespace where the target pod resides.
    namespace: String,
    /// The name of the target pod.
    pod_name: String,
}

impl Configurator {
    /// Creates a new `Configurator` instance.
    ///
    /// # Arguments
    ///
    /// * `api` - A Kubernetes API client configured for `Pod` resources.
    /// * `namespace` - The Kubernetes namespace where the target pod is
    ///   located.
    /// * `pod_name` - The name of the target pod.
    ///
    /// # Returns
    ///
    /// A new `Configurator` instance.
    pub fn new(api: Api<Pod>, namespace: impl Into<String>, pod_name: impl Into<String>) -> Self {
        Self { api, namespace: namespace.into(), pod_name: pod_name.into() }
    }

    /// Uploads an SSH public key to the `authorized_keys` file within the
    /// target pod's `~/.ssh` directory.
    ///
    /// This function executes a series of shell commands on the remote pod to:
    /// 1. Create the `~/.ssh` directory if it doesn't exist.
    /// 2. Set appropriate permissions (700 for `~/.ssh`, 600 for
    ///    `authorized_keys`).
    /// 3. Append the provided `ssh_public_key` to `~/.ssh/authorized_keys`.
    /// 4. Sort and deduplicate entries in `authorized_keys`.
    ///
    /// # Arguments
    ///
    /// * `ssh_public_key` - The SSH public key to be uploaded, typically in
    ///   `ssh-rsa` or `ssh-ed25519` format. This type must implement
    ///   `fmt::Display`.
    ///
    /// # Errors
    ///
    /// Returns an `Err` if:
    /// - There is an issue attaching to the pod or executing the commands
    ///   (e.g., pod not found, permission issues). This will be wrapped in an
    ///   `error::UploadSshKeySnafu`.
    pub async fn upload_ssh_key<P>(&self, ssh_public_key: P) -> Result<(), Error>
    where
        P: fmt::Display,
    {
        let Self { api, namespace, pod_name } = self;

        // We use a single shell command to:
        // 1. Create .ssh directory
        // 2. Append the key to authorized_keys
        // 3. Set correct permissions (SSH is picky about 700/600)
        let auth_command = [
            "sh".to_string(),
            "-c".to_string(),
            [
                "mkdir -p ~/.ssh",
                "chmod 700 ~/.ssh",
                &format!("echo '{ssh_public_key}' >> ~/.ssh/authorized_keys"),
                "chmod 600 ~/.ssh/authorized_keys",
                "sort -u ~/.ssh/authorized_keys -o ~/.ssh/authorized_keys",
            ]
            .join(" && "),
        ];

        let attached = api
            .exec(pod_name, auth_command, &AttachParams::default())
            .await
            .with_context(|_| error::UploadSshKeySnafu {
                namespace: namespace.clone(),
                pod_name: pod_name.clone(),
            })?;

        // Wait for the command to complete. The output is ignored for this operation.
        let _unused = attached.join().await;

        Ok(())
    }
}
