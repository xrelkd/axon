use std::fmt;

use k8s_openapi::api::core::v1::Pod;
use kube::{Api, api::AttachParams};
use snafu::ResultExt;

use crate::cli::{Error, error};

pub struct SshConfigurator {
    api: Api<Pod>,
    namespace: String,
    pod_name: String,
}

impl SshConfigurator {
    pub fn new(api: Api<Pod>, namespace: impl Into<String>, pod_name: impl Into<String>) -> Self {
        Self { api, namespace: namespace.into(), pod_name: pod_name.into() }
    }

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

        // Wait for the command to complete
        let _unused = attached.join().await;

        Ok(())
    }
}
